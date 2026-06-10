//! Advisor mode: reads game state JSON from stdin, prints best discard to stdout.
//! Format:
//!   INPUT:  {"hand":[0,4,8,...], "dora_indicators":[16,...], "seat_wind":27, "round_wind":27}
//!   OUTPUT: {"discard":44, "shanten":1, "display":"2m", "hand_after":"...", "eval":[...]}

use crate::ai::DiscardEval;
use crate::shanten;
use crate::tactics::TacticsAi;
use crate::state::GameState;
use crate::tile::{Tile136, t136_to_t34, tile136_display, tiles136_display};
use serde::Deserialize;
use std::io::{self, Read, Write};

#[derive(Debug, Deserialize)]
struct AdviceInput {
    hand: Vec<Tile136>,
    dora_indicators: Option<Vec<Tile136>>,
    seat_wind: Option<u8>,
    round_wind: Option<u8>,
    /// Whether user declared riichi
    riichi: Option<bool>,
}

pub fn run_advisor() -> anyhow::Result<()> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let advice: AdviceInput = serde_json::from_str(input.trim())?;

    let mut state = GameState::new(0, 0);
    state.bot_mut().hand_136 = advice.hand.clone();
    if let Some(ref dora) = advice.dora_indicators {
        state.dora_indicators = dora.clone();
    }
    if let Some(rw) = advice.round_wind {
        state.round_wind = rw;
    }
    if advice.riichi.unwrap_or(false) {
        state.bot_riichi = true;
        state.bot_mut().is_riichi = true;
    }

    let ai = TacticsAi::default();
    let evals = ai.evaluate_all_discards(&state);

    // Deduplicate by tile34 display: keep best-scoring tile136 for each display name
    let mut deduped: Vec<&DiscardEval> = Vec::new();
    for e in evals.iter() {
        let name = tile136_display(e.tile);
        if let Some(existing) = deduped.iter_mut().find(|d| tile136_display(d.tile) == name) {
            if e.score() < existing.score() {
                *existing = e;
            }
        } else {
            deduped.push(e);
        }
    }
    deduped.sort_by(|a, b| a.score().partial_cmp(&b.score()).unwrap_or(std::cmp::Ordering::Equal));

    let best = deduped.first().unwrap();

    // Format output — remove exactly one copy of best tile from hand
    let mut hand_after: Vec<Tile136> = advice.hand.clone();
    if let Some(pos) = hand_after.iter().position(|&t| t == best.tile) {
        hand_after.remove(pos);
    }

    let output = serde_json::json!({
        "discard": best.tile,
        "display": tile136_display(best.tile),
        "shanten": best.shanten,
        "ukeire": best.ukeire,
        "hand_after": hand_after.iter().map(|&t| tile136_display(t)).collect::<Vec<_>>().join(""),
        "hand_after_sorted": tiles136_display(&hand_after),
        "dora_in_hand": best.dora_count,
        "is_dangerous": best.is_dangerous,
        "score": best.score(),
        "top_candidates": deduped.iter()
            .take(5)
            .map(|e| serde_json::json!({
                "tile": tile136_display(e.tile),
                "shanten": e.shanten,
                "ukeire": e.ukeire,
                "dora": e.dora_count,
                "dangerous": e.is_dangerous,
                "score": e.score(),
            }))
            .collect::<Vec<_>>(),
    });

    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
