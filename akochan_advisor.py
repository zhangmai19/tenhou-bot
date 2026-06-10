#!/usr/bin/env python3
"""
Akochan Advisor — one-shot: reads game context from stdin, prints recommendation.

Keeps akochan stateless — bridge.js tracks the full game history and
rebuilds the mjai event stream for each turn.

Input (stdin JSON):
{
  "hand": [136,...14 tiles...],
  "dora": [136,...],         // all dora indicators this round
  "discards": [[136,...], [136,...], [136,...], [136,...]],  // per-player discards
  "riichi": [bool,...4],     // which players are in riichi
  "round": 0,                // round number (0=東1, 3=東4, 4=南1)
  "oya": 0,                  // dealer seat
}
"""

import sys, json, subprocess, os

AKOCHAN_DIR = "/tmp/akochan"
TACTICS = os.path.join(AKOCHAN_DIR, "tactics.json")
WRAPPER = os.path.join(AKOCHAN_DIR, "run_akochan.sh")

A34_STR = {
    0:"1m",1:"2m",2:"3m",3:"4m",4:"5m",5:"6m",6:"7m",7:"8m",8:"9m",
    9:"1p",10:"2p",11:"3p",12:"4p",13:"5p",14:"6p",15:"7p",16:"8p",17:"9p",
    18:"1s",19:"2s",20:"3s",21:"4s",22:"5s",23:"6s",24:"7s",25:"8s",26:"9s",
    27:"E",28:"S",29:"W",30:"N",31:"P",32:"F",33:"C",
}
RED = {16:"5mr",52:"5pr",88:"5sr"}

def t2s(t):
    if t in RED: return RED[t]
    return A34_STR.get(t//4,"?")

def build_events(data):
    """Build complete mjai event stream from current game state."""
    events = []

    hand = data.get("hand", [])
    if len(hand) < 14: return events

    dora = data.get("dora", [])
    discards = data.get("discards", [[],[],[],[]])
    riichi = data.get("riichi", [False]*4)
    round_num = data.get("round", 0)
    oya = data.get("oya", 0)

    # start_game
    events.append(json.dumps({
        "type":"start_game","names":["P0","P1","P2","P3"],"kyoku_first":0,"aka_flag":True
    }))

    # round info
    bakaze = "E" if round_num < 4 else "S"
    kyoku = (round_num % 4) + 1
    dora_str = t2s(dora[0]) if dora else "1m"

    # initial hand (first 13) + dummy opponent hands
    hand_13 = [t2s(t) for t in hand[:13]]

    events.append(json.dumps({
        "type":"start_kyoku",
        "bakaze":bakaze,
        "dora_marker":dora_str,
        "kyoku":kyoku,"honba":0,"kyotaku":0,"oya":oya,
        "scores":[25000,25000,25000,25000],
        "tehais":[hand_13,["?"]*13,["?"]*13,["?"]*13]
    }))

    # Replay opponent discards in order
    # We interleave: for each turn, one player draws then discards
    # Since we don't know the exact interleaving, feed all opponent discards
    # in a reasonable order (by seat, alternating)
    max_d = max((len(d) for d in discards), default=0)
    for turn in range(max_d):
        for pid in range(4):
            if pid == 0 or len(discards[pid]) <= turn:
                continue
            t = discards[pid][turn]
            # Skip if this tile is actually in our hand (it's OUR discard, not opponent's)
            if pid == 0:
                continue
            events.append(json.dumps({
                "type":"tsumo","actor":pid,"pai":"?"
            }))
            events.append(json.dumps({
                "type":"dahai","actor":pid,"pai":t2s(t),"tsumogiri":False
            }))

    # Dora events beyond the first
    for d in dora[1:]:
        events.append(json.dumps({"type":"dora","dora_marker":t2s(d)}))

    # Reach events
    for pid in range(4):
        if riichi[pid]:
            events.append(json.dumps({"type":"reach","actor":pid}))
            events.append(json.dumps({"type":"reach_accepted","actor":pid}))

    # Our tsumo (last tile in hand is the drawn one)
    drawn = t2s(hand[-1])
    events.append(json.dumps({"type":"tsumo","actor":0,"pai":drawn}))

    return events


def run_akochan(events):
    env = os.environ.copy()
    env["LD_LIBRARY_PATH"] = AKOCHAN_DIR
    input_text = "\n".join(events) + "\n"

    try:
        result = subprocess.run(
            [WRAPPER, "pipe_detailed", TACTICS, "0"],
            input=input_text, capture_output=True, text=True, timeout=30,
            cwd=AKOCHAN_DIR, env=env
        )
        if result.returncode != 0 or not result.stdout.strip():
            return None
        return json.loads(result.stdout.strip())
    except Exception as e:
        return None


def best_action(evaluations):
    if not evaluations: return None
    best = max(evaluations, key=lambda e: e.get("review",{}).get("pt_exp_after",-999), default=None)
    if not best: return None
    m = best.get("moves",[{}])[0]
    sorted_evals = sorted(evaluations, key=lambda e: -e.get("review",{}).get("pt_exp_after",-999))
    return {
        "type": m.get("type","?"),
        "pai": m.get("pai",""),
        "tsumogiri": m.get("tsumogiri",False),
        "pt_exp_after": best.get("review",{}).get("pt_exp_after",0),
        "candidates": [{
            "pai": e["moves"][0].get("pai","?"),
            "type": e["moves"][0].get("type","?"),
            "tsumogiri": e["moves"][0].get("tsumogiri",False),
            "pt": e.get("review",{}).get("pt_exp_after",-999),
        } for e in sorted_evals[:6]]
    }


if __name__ == "__main__":
    try:
        data = json.loads(sys.stdin.read())
    except: sys.exit(1)

    events = build_events(data)
    if len(events) < 3:
        print(json.dumps({"error":"not enough context"}))
        sys.exit(0)

    evals = run_akochan(events)
    if evals:
        result = best_action(evals)
        if result:
            print(json.dumps(result))
            sys.exit(0)

    print(json.dumps({"error":"no recommendation"}))
