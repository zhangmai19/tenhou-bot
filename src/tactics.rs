//! Default tactics implementation — the rule-based decision engine.
//!
//! This is the place to inject your own understanding. The decision flow:
//! 1. Win if possible (tsumo/ron)
//! 2. Call meld if it significantly improves the hand
//! 3. Declare riichi if tenpai and conditions are good
//! 4. Discard the tile that maximizes tile efficiency
//!
//! ## How to add your own strategy:
//!
//! Look for the **CUSTOMIZE** comments throughout this file. Each marks a point
//! where you can adjust weights, add conditions, or change behavior.
//!
//! The key functions to modify:
//! - `evaluate_discard()` — add bonus/penalty to each discard candidate
//! - `should_riichi()` — change when to declare riichi
//! - `try_call_meld()` — change when to call chi/pon
//! - `should_call_kan()` — change kan behavior

use crate::ai::{AiStrategy, DiscardEval, MeldDecision};
use crate::meld::MeldType;
use crate::shanten::{self, best_discard_for_shanten};
use crate::state::{GameState, is_safe_tile};
use crate::tile::{self, Tile136, Tile34, t136_to_t34, tile34_suit, tile34_number, tile34_is_yaochuu};

/// The default tactics-based AI.
///
/// You can customize behavior by setting these fields.
pub struct TacticsAi {
    /// How much to value ukeire (tile acceptance count). Default: 1.0
    pub ukeire_weight: f64,
    /// How much to value dora retention. Default: 0.5
    pub dora_weight: f64,
    /// How much to penalize dangerous discards. Default: 3.0
    pub danger_penalty: f64,
    /// How much to value keeping safe tiles for folding. Default: 0.0
    /// Increase this when you want the bot to play more defensively.
    pub defense_weight: f64,
    /// Minimum shanten improvement to call a meld. Default: 2 (only call if it
    /// reduces shanten by at least 2, unless already near tenpai)
    pub min_shanten_improve_for_meld: i8,
    /// Whether to call melds at all. Set to false for menzen-focused play.
    pub allow_melds: bool,
    /// Whether to declare riichi when tenpai. Set to false for dama-ten.
    pub allow_riichi: bool,
    /// Minimum ukeire for riichi (to avoid bad waits). Default: 4
    pub min_ukeire_for_riichi: u32,
    /// Whether to fold (play defensively) when an opponent is in riichi
    pub fold_against_riichi: bool,
}

impl Default for TacticsAi {
    fn default() -> Self {
        TacticsAi {
            ukeire_weight: 1.0,
            dora_weight: 0.5,
            danger_penalty: 3.0,
            defense_weight: 0.0,
            min_shanten_improve_for_meld: 2,
            allow_melds: true,
            allow_riichi: true,
            min_ukeire_for_riichi: 4,
            fold_against_riichi: true,
        }
    }
}

impl AiStrategy for TacticsAi {
    fn choose_discard(&self, state: &GameState) -> Tile136 {
        let bot = state.bot();
        let hand = &bot.hand_136;

        if hand.is_empty() {
            log::error!("No tiles in hand to discard!");
            return 0;
        }

        // Evaluate each possible discard
        let evals = self.evaluate_all_discards(state);

        // Pick the best (lowest score)
        if let Some(best) = evals.iter().min_by(|a, b| {
            a.score().partial_cmp(&b.score()).unwrap_or(std::cmp::Ordering::Equal)
        }) {
            log::info!(
                "Discard choice: {} (shanten={}, ukeire={}, dora={}, dangerous={}, score={:.2})",
                tile::tile136_display(best.tile),
                best.shanten,
                best.ukeire,
                best.dora_count,
                best.is_dangerous,
                best.score(),
            );
            best.tile
        } else {
            // Fallback: discard the last (most recently drawn) tile
            log::warn!("No good discard found, using last tile");
            state.last_drawn_tile.unwrap_or(hand[hand.len() - 1])
        }
    }

    fn should_riichi(&self, state: &GameState) -> (bool, Option<Tile136>) {
        if !self.allow_riichi || state.bot_riichi {
            return (false, None);
        }

        let bot = state.bot();
        let hand = &bot.hand_136;

        // Must be menzen (no open melds)
        let has_open_melds = bot.melds.iter().any(|m| {
            matches!(m.meld_type, MeldType::Chi | MeldType::Pon | MeldType::Daiminkan)
        });
        if has_open_melds {
            return (false, None);
        }

        let info = shanten::calc_shanten_detailed(hand);
        if !info.is_tenpai() {
            return (false, None);
        }

        // In tenpai — check if ukeire is good enough
        if info.ukeire_count < self.min_ukeire_for_riichi {
            log::info!("Tenpai but ukeire too low ({}) for riichi, playing dama", info.ukeire_count);
            return (false, None);
        }

        // **CUSTOMIZE**: Add your riichi conditions here.
        // For example:
        // - Don't riichi if someone else is already in riichi
        // - Don't riichi with a bad wait (tanki, penchan)
        // - Always riichi if it's the last round and you need points

        // Find which tile to discard for riichi
        // The riichi declaration tile is the one we would discard to stay tenpai
        let best = best_discard_for_shanten(hand);
        if let Some((tile, _, _)) = best {
            log::info!("Declaring riichi, discarding {}", tile::tile136_display(tile));
            (true, Some(tile))
        } else {
            (false, None)
        }
    }

    fn should_call_kan(&self, _state: &GameState, _drawn_tile: Tile136, _from_opponent: bool) -> (u8, Option<Tile136>) {
        // **CUSTOMIZE**: Add your kan logic here.
        // For now, conservative: never kan (to avoid giving opponents extra dora)
        (0, None)
    }

    fn try_call_meld(&self, state: &GameState, discard_tile: Tile136, can_chi: bool) -> Option<MeldDecision> {
        if !self.allow_melds {
            return None;
        }

        if state.bot_riichi {
            return None; // Can't call melds in riichi
        }

        let bot = state.bot();
        let hand = &bot.hand_136;
        let tile34 = t136_to_t34(discard_tile);

        // **CUSTOMIZE**: Adjust meld calling strategy here.
        // The current logic is: only call if it improves shanten significantly.

        // Check pon
        let count_in_hand = hand.iter().filter(|&&t| t136_to_t34(t) == tile34).count();
        if count_in_hand >= 2 {
            // Can pon — evaluate if it helps
            let mut hypothetical_hand = hand.clone();
            // Remove 2 copies from hand
            let mut removed = 0;
            hypothetical_hand.retain(|&t| {
                if removed < 2 && t136_to_t34(t) == tile34 {
                    removed += 1;
                    false
                } else {
                    true
                }
            });
            // Add the called tile
            hypothetical_hand.push(discard_tile);

            let current_info = shanten::calc_shanten_detailed(hand);
            let after_info = shanten::calc_shanten_detailed(&hypothetical_hand);

            let improvement = current_info.shanten - after_info.shanten;
            if improvement >= self.min_shanten_improve_for_meld || (current_info.shanten <= 1 && improvement >= 1) {
                // Find best discard after pon
                if let Some((discard, _, _)) = best_discard_for_shanten(&hypothetical_hand) {
                    log::info!(
                        "Calling PON on {} — shanten {}→{}, then discard {}",
                        tile::tile136_display(discard_tile),
                        current_info.shanten,
                        after_info.shanten,
                        tile::tile136_display(discard),
                    );
                    return Some(MeldDecision {
                        meld_type: 1, // pon
                        hand_tiles: hand.iter()
                            .filter(|&&t| t136_to_t34(t) == tile34)
                            .take(2)
                            .copied()
                            .collect(),
                        discard_after: discard,
                    });
                }
            }
        }

        // Check chi
        if can_chi && tile34_suit(tile34) != tile::Suit::Honor {
            let num = tile34_number(tile34);
            // Try all possible chi patterns
            let patterns = [
                (num.wrapping_sub(2), num.wrapping_sub(1)), // XX-1, XX, tile
                (num.wrapping_sub(1), num + 1),             // XX, tile, XX+1
                (num + 1, num + 2),                        // tile, XX+1, XX+2
            ];

            let suit = tile34_suit(tile34);
            for &(n1, n2) in &patterns {
                if n1 < 1 || n2 < 1 || n1 > 9 || n2 > 9 {
                    continue;
                }
                let t1 = suit_number_to_t34(suit, n1);
                let t2 = suit_number_to_t34(suit, n2);
                if t1.is_none() || t2.is_none() {
                    continue;
                }
                let t1 = t1.unwrap();
                let t2 = t2.unwrap();

                let has_t1 = hand.iter().any(|&t| t136_to_t34(t) == t1);
                let has_t2 = hand.iter().any(|&t| t136_to_t34(t) == t2);
                if has_t1 && has_t2 {
                    // Can chi — evaluate
                    let mut hypothetical_hand = hand.clone();
                    // Remove t1 and t2
                    let mut removed_t1 = false;
                    let mut removed_t2 = false;
                    hypothetical_hand.retain(|&t| {
                        let tt = t136_to_t34(t);
                        if !removed_t1 && tt == t1 {
                            removed_t1 = true;
                            false
                        } else if !removed_t2 && tt == t2 {
                            removed_t2 = true;
                            false
                        } else {
                            true
                        }
                    });
                    hypothetical_hand.push(discard_tile);

                    let current_info = shanten::calc_shanten_detailed(hand);
                    let after_info = shanten::calc_shanten_detailed(&hypothetical_hand);
                    let improvement = current_info.shanten - after_info.shanten;

                    if improvement >= self.min_shanten_improve_for_meld || (current_info.shanten <= 1 && improvement >= 1) {
                        if let Some((discard, _, _)) = best_discard_for_shanten(&hypothetical_hand) {
                            log::info!(
                                "Calling CHI on {} — shanten {}→{}, then discard {}",
                                tile::tile136_display(discard_tile),
                                current_info.shanten,
                                after_info.shanten,
                                tile::tile136_display(discard),
                            );
                            // Find the actual tile136 for t1 and t2 in hand
                            let ht1 = hand.iter().find(|&&t| t136_to_t34(t) == t1).copied().unwrap_or(0);
                            let ht2 = hand.iter().find(|&&t| t136_to_t34(t) == t2).copied().unwrap_or(0);
                            return Some(MeldDecision {
                                meld_type: 3, // chi
                                hand_tiles: vec![ht1, ht2],
                                discard_after: discard,
                            });
                        }
                    }
                }
            }
        }

        None
    }

    fn should_tsumo(&self, state: &GameState) -> bool {
        // In riichi: we already committed, always declare tsumo
        if state.bot_riichi {
            return true;
        }
        // **CUSTOMIZE**: In dama, you might skip tsumo for a better hand
        true
    }

    fn should_ron(&self, _state: &GameState, _discard_tile: Tile136) -> bool {
        // Always ron — even in dama
        true
    }

    fn should_kyushu_kyuhai(&self, state: &GameState) -> bool {
        // Kyushu kyuhai: 9+ terminals/honors in starting hand
        let bot = state.bot();
        let count = bot.hand_136.iter()
            .filter(|&&t| tile34_is_yaochuu(t136_to_t34(t)))
            .count();
        count >= 9
    }
}

impl TacticsAi {
    /// Evaluate all possible discards for the current hand.
    /// This is the main function to customize for injecting your own strategy.
    pub fn evaluate_all_discards(&self, state: &GameState) -> Vec<DiscardEval> {
        let bot = state.bot();
        let hand = &bot.hand_136;

        // If we only have one tile or are in riichi, just discard the drawn tile
        if hand.len() <= 1 {
            return vec![DiscardEval {
                tile: hand[0],
                shanten: 0,
                ukeire: 0,
                dora_count: 0,
                is_dangerous: false,
                custom_score: 0.0,
            }];
        }

        if state.bot_riichi {
            // In riichi: must tsumogiri (discard the drawn tile)
            let last = state.last_drawn_tile.unwrap_or(hand[hand.len() - 1]);
            return vec![DiscardEval {
                tile: last,
                shanten: 0,
                ukeire: 0,
                dora_count: 0,
                is_dangerous: false,
                custom_score: 0.0,
            }];
        }

        let dora_tiles = state.dora_tiles();
        let _visible = state.visible_counts();
        let has_opponent_riichi = state.players.iter().any(|p| p.is_riichi);

        let mut evaluations = Vec::with_capacity(hand.len());

        for (i, &tile) in hand.iter().enumerate() {
            let mut new_hand: Vec<Tile136> = hand.clone();
            new_hand.remove(i);

            let info = shanten::calc_shanten_detailed(&new_hand);

            // Count dora in hand after discard
            let dora_count = new_hand.iter()
                .filter(|&&t| dora_tiles.contains(&t136_to_t34(t)))
                .count()
                + new_hand.iter().filter(|&&t| tile::is_aka(t)).count();

            // Check if this tile is dangerous
            let mut is_dangerous = false;
            if self.fold_against_riichi && has_opponent_riichi {
                for p in &state.players {
                    if p.is_riichi && !p.is_bot {
                        if !is_safe_tile(tile, p, &p.discards) {
                            is_dangerous = true;
                            break;
                        }
                    }
                }
            }

            // **CUSTOMIZE**: Add your own evaluation logic here.

            let mut custom_score = 0.0;

            let tile34 = t136_to_t34(tile);

            // (1) Penalize breaking the ONLY pair in the hand
            // A hand with no pair is significantly worse than iishanten-with-pair.
            let count_this_tile = hand.iter().filter(|&&t| t136_to_t34(t) == tile34).count();
            if count_this_tile == 2 {
                // This tile is part of a pair. Check if it's the ONLY pair.
                let mut has_other_pair = false;
                for t34 in 0..34u8 {
                    if t34 == tile34 { continue; }
                    let cnt = hand.iter().filter(|&&t| t136_to_t34(t) == t34).count();
                    if cnt >= 2 { has_other_pair = true; break; }
                }
                if !has_other_pair {
                    // Breaking the sole pair — severe penalty
                    custom_score += 5.0;
                }
            }

            // (2) Prefer keeping dora
            if dora_tiles.contains(&tile34) || tile::is_aka(tile) {
                custom_score += self.dora_weight;
            }

            // (3) In fold mode: prefer safe discards
            if has_opponent_riichi && self.fold_against_riichi {
                if is_dangerous && info.shanten > 1 {
                    custom_score -= self.defense_weight * 0.5;
                }
                if !is_dangerous && info.shanten > 1 {
                    custom_score += self.defense_weight;
                }
            }

            // (3) Prefer discarding isolated tiles (those with no adjacent tiles)
            // This is already handled by shanten, but we can add a small bias
            let _adjacent_count = if tile34_suit(tile34) != tile::Suit::Honor {
                let mut count = 0;
                for n in [tile34.wrapping_sub(1), tile34 + 1] {
                    if new_hand.iter().any(|&t| t136_to_t34(t) == n) {
                        count += 1;
                    }
                }
                count
            } else {
                0
            };
            if _adjacent_count == 0 && info.shanten == 0 {
                // Isolated tile in tenpai: prefer keeping over breaking a pair/taatsu
                // Actually, this is wrong. In tenpai we need to keep the tenpai shape.
            }

            // (4) Near tenpai: consider what wait the discard leaves
            // **CUSTOMIZE**: Add wait quality evaluation here

            evaluations.push(DiscardEval {
                tile,
                shanten: info.shanten,
                ukeire: info.ukeire_count,
                dora_count,
                is_dangerous,
                custom_score,
            });
        }

        evaluations
    }
}

/// Convert suit and number to tile34
fn suit_number_to_t34(suit: tile::Suit, number: u8) -> Option<Tile34> {
    if number < 1 || number > 9 {
        return None;
    }
    match suit {
        tile::Suit::Man => Some(number - 1),
        tile::Suit::Pin => Some(9 + number - 1),
        tile::Suit::Sou => Some(18 + number - 1),
        tile::Suit::Honor => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tactics_default() {
        let ai = TacticsAi::default();
        assert!(ai.allow_melds);
        assert!(ai.allow_riichi);
        assert_eq!(ai.min_shanten_improve_for_meld, 2);
    }
}
