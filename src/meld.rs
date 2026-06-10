//! Meld (fuuro) representation for called tile groups.

use crate::tile::{Tile136, Tile34, t136_to_t34, tiles136_display};

/// A called meld (open or closed).
#[derive(Debug, Clone)]
pub struct Meld {
    pub meld_type: MeldType,
    /// All tiles in this meld (in 136 format)
    pub tiles: Vec<Tile136>,
    /// Which tile was called from opponent
    pub called_tile: Tile136,
    /// Which opponent was it called from (0-3, where 0=self)
    pub from_whom: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeldType {
    /// Open chi (sequence)
    Chi,
    /// Open pon (triplet)
    Pon,
    /// Open kan (daiminkan — called from opponent's discard)
    Daiminkan,
    /// Closed kan (ankan)
    Ankan,
    /// Added kan (kakan — adding to an existing pon)
    Kakan,
}

impl Meld {
    /// Get all tiles as tile34
    pub fn tiles34(&self) -> Vec<Tile34> {
        self.tiles.iter().map(|&t| t136_to_t34(t)).collect()
    }

    /// Number of tiles consumed from hand
    pub fn consumed_count(&self) -> usize {
        match self.meld_type {
            MeldType::Chi => 2,
            MeldType::Pon => 2,
            MeldType::Daiminkan => 3,
            MeldType::Ankan => 4,
            MeldType::Kakan => 1, // adds 1 to existing pon
        }
    }

    /// The tile type (34) of this meld's primary tile
    pub fn tile_type(&self) -> Tile34 {
        t136_to_t34(self.tiles[0])
    }

    /// Display for logging
    pub fn display(&self) -> String {
        let type_str = match self.meld_type {
            MeldType::Chi => "チー",
            MeldType::Pon => "ポン",
            MeldType::Daiminkan => "大明槓",
            MeldType::Ankan => "暗槓",
            MeldType::Kakan => "加槓",
        };
        format!("{} {}", type_str, tiles136_display(&self.tiles))
    }
}
