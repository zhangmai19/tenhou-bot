//! AI interface — traits that define how the bot makes decisions.
//!
//! This is where you inject your own understanding of mahjong strategy.

use crate::state::GameState;
use crate::tile::Tile136;

/// The main AI trait. Implement this to customize the bot's behavior.
///
/// The default implementation in `tactics.rs` uses:
/// - Shanten-based tile efficiency for discard selection
/// - Basic safety detection for dealing into opponents
/// - Simple meld calling logic
///
/// To add your own strategy:
/// 1. Create a struct that wraps `TacticsAi` or implements `AiStrategy` directly
/// 2. Override specific methods to inject your understanding
/// 3. The `evaluate_discard` method is the main hook for custom discard logic
pub trait AiStrategy {
    /// Choose which tile to discard.
    /// Called when it's the bot's turn and no meld/riichi/tsumo was chosen.
    fn choose_discard(&self, state: &GameState) -> Tile136;

    /// Decide whether to call riichi after drawing.
    /// Returns (should_riichi, tile_to_discard_if_riichi).
    fn should_riichi(&self, state: &GameState) -> (bool, Option<Tile136>);

    /// Decide whether to call a kan.
    /// `drawn_tile` is the tile just drawn.
    /// `from_opponent` is whether this kan would use an opponent's discard.
    /// Returns (kan_type, tile_to_kan).
    /// kan_type: 0=none, 1=ankan(from hand), 2=daiminkan(from discard), 3=kakan(add to pon)
    fn should_call_kan(&self, state: &GameState, drawn_tile: Tile136, from_opponent: bool) -> (u8, Option<Tile136>);

    /// Decide whether to call a meld (chi/pon) on an opponent's discard.
    /// `discard_tile` is what was just discarded.
    /// `can_chi` is whether chi is possible (only from kamicha / left player).
    /// Returns the meld to call, or None to pass.
    fn try_call_meld(&self, state: &GameState, discard_tile: Tile136, can_chi: bool) -> Option<MeldDecision>;

    /// Decide whether to declare tsumo (win by self-draw).
    fn should_tsumo(&self, state: &GameState) -> bool;

    /// Decide whether to declare ron (win by opponent's discard).
    fn should_ron(&self, state: &GameState, discard_tile: Tile136) -> bool;

    /// Decide whether to declare kyushu kyuhai (9 terminals — abortive draw).
    fn should_kyushu_kyuhai(&self, state: &GameState) -> bool;
}

/// Decision for calling a meld
#[derive(Debug, Clone)]
pub struct MeldDecision {
    /// Type of meld: 1=pon, 3=chi, 2=daiminkan
    pub meld_type: u8,
    /// Tiles from hand to include (in 136 format)
    pub hand_tiles: Vec<Tile136>,
    /// Which tile to discard after calling the meld
    pub discard_after: Tile136,
}

/// Simple pre-computed evaluation of each possible discard.
#[derive(Debug, Clone)]
pub struct DiscardEval {
    /// The tile to discard (136 format)
    pub tile: Tile136,
    /// Shanten number after discarding this tile
    pub shanten: i8,
    /// Number of tiles that would improve shanten
    pub ukeire: u32,
    /// Number of dora in hand after discarding
    pub dora_count: usize,
    /// Whether this tile is "dangerous" (might deal into an opponent)
    pub is_dangerous: bool,
    /// Custom score from user strategy (higher = better)
    /// Set this in your custom evaluation to influence the final decision.
    pub custom_score: f64,
}

impl DiscardEval {
    /// Calculate a composite score. Lower = better discard.
    /// This is where you tune weights based on your understanding.
    pub fn score(&self) -> f64 {
        let mut s = 0.0;

        // Shanten: primary factor — always reduce shanten first
        s += self.shanten as f64 * 10.0;

        // Ukeire: among same shanten, prefer more tile acceptance
        s -= (self.ukeire as f64) * 0.1;

        // Dora: prefer keeping dora
        s += (1.0 - self.dora_count as f64 * 0.5).max(0.0);

        // Safety: penalize dangerous discards
        if self.is_dangerous {
            s += 3.0;
        }

        // Custom strategy adjustment
        s += self.custom_score;

        s
    }
}
