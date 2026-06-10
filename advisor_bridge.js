// Tenhou Advisor — Node.js directly manages akochan pipe_detailed process.
// No Python middle layer. Async stdin/stdout, no deadlocks.
//
// Protocol conversion:
//   Tenhou WS {"tag":"T78"} → mjai {"type":"tsumo","actor":0,"pai":"2m"}
//   akochan pipe_detailed reads mjai, outputs JSON evaluations on actor=0 tsumo

const { chromium } = require('playwright');
const { spawn } = require('child_process');
const readline = require('readline');

// ── Tile conversion: tile136 → akochan string ────────────────────────────

const RED = { 16: '5mr', 52: '5pr', 88: '5sr' };
const T34 = [
  '1m','2m','3m','4m','5m','6m','7m','8m','9m',
  '1p','2p','3p','4p','5p','6p','7p','8p','9p',
  '1s','2s','3s','4s','5s','6s','7s','8s','9s',
  'E','S','W','N','P','F','C',
];
function toAko(t136) {
  if (RED[t136]) return RED[t136];
  return T34[Math.floor(t136/4)] || '?';
}

// ── Akochan process ──────────────────────────────────────────────────────

let akochan = null;
let akochanStdout = null;
let pendingResolve = null;
let pendingTimer = null;
let responseLines = [];

function akochanStart() {
  if (akochan && !akochan.killed) return;

  akochan = spawn('/tmp/akochan/system.exe', [
    'pipe_detailed',
    '/tmp/akochan/tactics.json',
    '0'
  ], {
    cwd: '/tmp/akochan',
    env: { ...process.env, LD_LIBRARY_PATH: '/tmp/akochan' },
    stdio: ['pipe', 'pipe', 'pipe'],
  });

  akochanStdout = readline.createInterface({ input: akochan.stdout });

  akochanStdout.on('line', (line) => {
    const trimmed = line.trim();
    if (!trimmed) return;

    if (pendingResolve) {
      // Collect lines until we see a complete JSON array response
      responseLines.push(trimmed);
      // Try to parse the accumulated response as JSON
      const combined = responseLines.join('');
      try {
        JSON.parse(combined);
        // It's valid JSON! Resolve.
        const resolveFn = pendingResolve;
        pendingResolve = null;
        if (pendingTimer) { clearTimeout(pendingTimer); pendingTimer = null; }
        responseLines = [];
        resolveFn(combined);
      } catch(e) {
        // Not valid JSON yet — wait for more lines
      }
    }
  });

  akochan.stderr.on('data', (data) => {
    // Log stderr for debugging
    const msg = data.toString().trim();
    if (msg) console.error(`   [akochan stderr] ${msg.substring(0, 200)}`);
  });

  akochan.on('close', (code) => {
    akochan = null;
    if (pendingResolve) {
      pendingResolve = null;
      console.log(`\n⚠️  Akochan exited (code ${code}) — restarting...`);
    }
  });
}

function akochanWrite(json) {
  if (!akochan || akochan.killed) akochanStart();
  akochan.stdin.write(json + '\n');
}

async function akochanRead(timeoutMs = 25000) {
  return new Promise((resolve) => {
    pendingResolve = resolve;
    responseLines = [];
    pendingTimer = setTimeout(() => {
      if (pendingResolve) {
        pendingResolve = null;
        resolve(null); // timeout → null
      }
    }, timeoutMs);
  });
}

async function akochanAsk(event) {
  akochanWrite(event);
  const raw = await akochanRead(25000);
  if (!raw) return null;
  try { return JSON.parse(raw); } catch(e) { return null; }
}

function parseBest(evaluations) {
  if (!evaluations || !Array.isArray(evaluations)) return null;
  const sorted = [...evaluations].sort((a, b) =>
    (b.review?.pt_exp_after || -999) - (a.review?.pt_exp_after || -999)
  );
  const best = sorted[0];
  const m = best?.moves?.[0] || {};
  return {
    type: m.type || '?',
    pai: m.pai || '',
    tsumogiri: m.tsumogiri || false,
    pt_exp_after: best?.review?.pt_exp_after || 0,
    candidates: sorted.slice(0, 6).map(e => ({
      pai: e.moves?.[0]?.pai || '?',
      type: e.moves?.[0]?.type || '?',
      tsumogiri: e.moves?.[0]?.tsumogiri || false,
      pt: e.review?.pt_exp_after || -999,
    })),
  };
}

// ── Game state ────────────────────────────────────────────────────────────

let st = {
  hand: [],
  dora: [],
  discards: [[],[],[],[]],
  round: 0,
  honba: 0,
  oya: 0,
  seat: 0,
  riichi: false,
  myName: null,
  // Has start_kyoku been sent to akochan this round?
  akochanInited: false,
};

function resetRound(haiStr, doraInd, seed) {
  st.hand = haiStr ? haiStr.split(',').map(Number).sort((a,b)=>a-b) : [];
  st.dora = doraInd > 0 ? [doraInd] : [];
  st.discards = [[],[],[],[]];
  st.riichi = false;
  st.akochanInited = false;
  if (seed) {
    const s = seed.split(',').map(Number);
    st.round = s[0] || 0;
    st.honba = s[1] || 0;
  }
}

// ── Helpers ───────────────────────────────────────────────────────────────

function tName(p) {
  if (p == null) return '?';
  return T34[Math.floor(p/4)] || '?';
}
function hStr(ts) { return ts.map(tName).join(''); }
function isRed(p) { return p===16||p===52||p===88; }

// Dora indicator → actual bonus tile
function doraToBonus(ind136) {
  const t = Math.floor(ind136/4);
  if (t < 9)  return (t===8 ? 0 : t+1) * 4;       // man: 9→1
  if (t < 18) return (t===17? 9 : t+1) * 4;       // pin: 9→1
  if (t < 27) return (t===26? 18: t+1) * 4;       // sou: 9→1
  // winds: E→S→W→N→E, dragons: P→F→C→P
  if (t < 31) return t===30? 27*4 : (t+1)*4;
  return t===33? 31*4 : (t+1)*4;
}

// ── Display recommendation ────────────────────────────────────────────────

function showRec(rec) {
  if (!rec || !rec.candidates) { console.log('   ⚠️  No recommendation'); return; }
  const b = rec.candidates[0];
  const ts = rec.tsumogiri ? 'TSUMOGIRI (摸切)' : 'DISCARD (手出し)';
  console.log(`\n╔══════════════════════════════════════╗`);
  console.log(`║  🤖 AKOCHAN                          ║`);
  console.log(`╠══════════════════════════════════════╣`);
  console.log(`║  ${ts.padEnd(34)}║`);
  console.log(`║  Cut: ${(rec.pai||'?').padEnd(30)}║`);
  console.log(`║  EV: ${(rec.pt_exp_after||0).toFixed(1).padEnd(31)}║`);
  console.log(`╠══════════════════════════════════════╣`);
  rec.candidates.slice(0, 6).forEach((c,i) => {
    console.log(`║   ${i===0?'★':' '} ${(c.pai||'?').padEnd(5)} ${c.tsumogiri?'摸':'手'} pt=${c.pt.toFixed(1).padEnd(8)}  ║`);
  });
  console.log(`╚══════════════════════════════════════╝`);
}

// ── Message handler ───────────────────────────────────────────────────────

async function handle(raw) {
  let d;
  try { d = JSON.parse(raw); } catch(_) { return; }
  const tag = d.tag || '';
  const ch = tag.charAt(0);
  const num = tag.length > 1 ? parseInt(tag.substring(1)) : null;

  // ── Auth / setup ────────────────────────────────────────────────────
  if (tag === 'HELO' && d.uname) {
    st.myName = decodeURIComponent(d.uname);
    console.log(`\n👤 ${st.myName}`);
    return;
  }
  if (tag === 'UN') {
    const ns = [d.n0,d.n1,d.n2,d.n3].map(s => s ? decodeURIComponent(s) : '?');
    if (st.myName) st.seat = ns.indexOf(st.myName);
    if (st.seat === -1) st.seat = 0;
    console.log(`   Players: ${ns.join(' | ')} (seat ${st.seat})`);
    return;
  }
  if (tag === 'TAIKYOKU') { st.oya = parseInt(d.oya)||0; console.log(`\n🀄 対局開始! oya=seat${st.oya}`); return; }
  if (tag === 'GO' || tag === 'GOK' || tag === 'NEXTREADY' || tag === 'BYE') return;

  // ── Round init ──────────────────────────────────────────────────────
  if (tag === 'INIT') {
    const seed = (d.seed||'').split(',').map(Number);
    resetRound(d.hai, seed[5]||0, d.seed);

    // End previous round in akochan if needed
    if (st.akochanInited) {
      akochanWrite(JSON.stringify({type:'end_kyoku'}));
    }

    // Start new game + kyoku in akochan
    const bakaze = st.round < 4 ? 'E' : 'S';
    const kyoku = (st.round % 4) + 1;
    const hand13 = st.hand.map(toAko); // initial 13 tiles
    const doraStr = toAko(st.dora[0] || 0);

    akochanWrite(JSON.stringify({
      type: 'start_game',
      names: ['P0','P1','P2','P3'],
      kyoku_first: 0, aka_flag: true
    }));
    akochanWrite(JSON.stringify({
      type: 'start_kyoku',
      bakaze, dora_marker: doraStr, kyoku, honba: st.honba, kyotaku: 0,
      oya: st.oya, scores: [25000,25000,25000,25000],
      tehais: [hand13, ['?','?','?','?','?','?','?','?','?','?','?','?','?'],
               ['?','?','?','?','?','?','?','?','?','?','?','?','?'],
               ['?','?','?','?','?','?','?','?','?','?','?','?','?']]
    }));
    st.akochanInited = true;

    console.log(`\n── ${rName()} ──`);
    console.log(`   Dora指示: ${tName(st.dora[0])} → 宝牌: ${tName(doraToBonus(st.dora[0]))} | Hand: ${hStr(st.hand)}`);
    return;
  }

  // ── Self draw ────────────────────────────────────────────────────────
  if (ch === 'T' && num !== null) {
    st.hand.push(num);
    console.log(`\n⬇  Draw: ${tName(num)}${isRed(num)?' (赤)':''} | 🤖...`);

    // Send tsumo to akochan → it will respond with evaluation
    const event = JSON.stringify({ type: 'tsumo', actor: 0, pai: toAko(num) });
    const evals = await akochanAsk(event);
    if (evals) {
      const rec = parseBest(evals);
      showRec(rec);
    } else {
      console.log('   ⚠️  akochan timeout');
    }
    return;
  }

  // ── Self discard ─────────────────────────────────────────────────────
  if ((ch === 'D' || ch === 'd') && num !== null) {
    const idx = st.hand.indexOf(num);
    if (idx >= 0) st.hand.splice(idx, 1);
    st.discards[st.seat].push(num);
    // Tell akochan
    akochanWrite(JSON.stringify({ type: 'dahai', actor: 0, pai: toAko(num), tsumogiri: ch === 'd' }));
    return;
  }

  // ── Opponent discard ─────────────────────────────────────────────────
  const oppMap = { 'E':1,'e':1, 'F':2,'f':2, 'G':3,'g':3 };
  if (oppMap[ch] && num !== null) {
    const pid = oppMap[ch];
    st.discards[pid].push(num);

    // Feed to akochan (opponent draws then discards)
    akochanWrite(JSON.stringify({ type: 'tsumo', actor: pid, pai: '?' }));
    akochanWrite(JSON.stringify({ type: 'dahai', actor: pid, pai: toAko(num), tsumogiri: ch === ch.toLowerCase() }));

    // Meld alerts (check locally)
    if (!st.riichi) {
      const t34 = Math.floor(num/4);
      const cnt = st.hand.filter(t => Math.floor(t/4) === t34).length;
      const canChi = (st.seat + 3) % 4 === pid;
      if (cnt >= 2) console.log(`   🀄 PON possible (${tName(num)})`);
      if (canChi && t34 < 27) {
        const ss = Math.floor(t34/9)*9, n = t34 - ss;
        for (const [a,b] of [[n-2,n-1],[n-1,n+1],[n+1,n+2]]) {
          if (a<0||b<0||a>8||b>8) continue;
          const tA = ss+a, tB = ss+b;
          if (st.hand.some(t => Math.floor(t/4)===tA) && st.hand.some(t => Math.floor(t/4)===tB)) {
            console.log(`   🍜 CHI possible (${tName(tA*4)}+${tName(tB*4)})`);
            break;
          }
        }
      }
    }
    return;
  }

  // ── Other events ─────────────────────────────────────────────────────
  if (tag === 'DORA') {
    const hai = typeof d.hai === 'number' ? d.hai : parseInt(d.hai);
    if (!isNaN(hai)) {
      st.dora.push(hai);
      akochanWrite(JSON.stringify({ type: 'dora', dora_marker: toAko(hai) }));
      console.log(`   🀄 Dora: ${tName(hai)}`);
    }
    return;
  }
  if (tag === 'REACH') {
    const who = parseInt(d.who)||0;
    if (who === st.seat) st.riichi = true;
    akochanWrite(JSON.stringify({ type: 'reach', actor: who }));
    akochanWrite(JSON.stringify({ type: 'reach_accepted', actor: who }));
    console.log(`   🔴 ${who===st.seat?'You':'P'+who} RIICHI!`);
    return;
  }
  if (tag === 'AGARI') {
    const w = parseInt(d.who), f = parseInt(d.fromWho);
    console.log(`\n🏆 P${w} wins${w!==f?` from P${f}`:' (tsumo)'}`);
    return;
  }
  if (tag === 'RYUUKYOKU') { console.log(`\n⏸️  Ryuukyoku`); return; }
}

function rName() {
  const w = st.round < 4 ? '東' : '南';
  return `${w}${(st.round%4)+1}局 ${st.honba}本場`;
}

// ── Main ──────────────────────────────────────────────────────────────────

(async () => {
  // Start akochan process
  akochanStart();
  console.log('🤖 Akochan engine loaded\n');

  const browser = await chromium.launch({ headless: false, args: ['--no-sandbox'] });
  const page = await browser.newPage();

  page.on('websocket', (ws) => {
    console.log(`🔌 ${ws.url()}\n`);
    ws.on('framereceived', (f) => handle(f.payload.toString()));
    ws.on('close', () => console.log('\n🔌 Disconnected'));
  });

  await page.goto('https://tenhou.net/3/', { waitUntil: 'domcontentloaded', timeout: 30000 });
  await page.waitForTimeout(2000);
  const ok = await page.$('button:has-text("OK")');
  if (ok) { await ok.click(); await page.waitForTimeout(500); }

  console.log('Ready! Login and play.\n───────────────────────────────────────────────\n');
  await new Promise(() => {});
})().catch(e => { console.error('Fatal:', e.message); process.exit(1); });
