//! Tile types and conversions for Japanese mahjong.
//!
//! Two representations:
//! - `Tile34` (0-33): The 34 distinct tile types
//! - `Tile136` (0-135): All 136 tiles (4 copies of each type)

// serde traits not needed for tile primitives

/// A tile in the 34-type representation.
/// 0-8: 1m-9m, 9-17: 1p-9p, 18-26: 1s-9s, 27-33: E,S,W,N,Haku,Hatsu,Chun
pub type Tile34 = u8;

/// A tile in the 136-type representation (as used by Tenhou protocol).
/// tile136 = tile34 * 4 + copy_index (0-3)
/// Red fives: 16 (5mr), 52 (5pr), 88 (5sr)
pub type Tile136 = u16;

/// Convert Tile136 to Tile34
#[inline]
pub fn t136_to_t34(t: Tile136) -> Tile34 {
    (t / 4) as Tile34
}

/// Convert Tile34 to base Tile136 (copy 0, i.e. non-red version)
#[inline]
pub fn t34_to_t136(t: Tile34) -> Tile136 {
    (t as Tile136) * 4
}

/// Check if a tile136 is a red five
#[inline]
pub fn is_red_five(t: Tile136) -> bool {
    matches!(t, 16 | 52 | 88)
}

/// Check if a tile136 is an aka (red) tile
#[inline]
pub fn is_aka(t: Tile136) -> bool {
    is_red_five(t)
}

/// Get the non-red version of a tile136
#[inline]
pub fn deaka(t: Tile136) -> Tile136 {
    if is_red_five(t) {
        t - 1
    } else {
        t
    }
}

/// Suit of a tile34
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suit {
    Man = 0,
    Pin = 1,
    Sou = 2,
    Honor = 3,
}

/// Get suit of a tile34
pub fn tile34_suit(t: Tile34) -> Suit {
    match t {
        0..=8 => Suit::Man,
        9..=17 => Suit::Pin,
        18..=26 => Suit::Sou,
        _ => Suit::Honor,
    }
}

/// Get the number within its suit (1-9 for suits, 1-7 for honors)
pub fn tile34_number(t: Tile34) -> u8 {
    match tile34_suit(t) {
        Suit::Man => t + 1,
        Suit::Pin => t - 9 + 1,
        Suit::Sou => t - 18 + 1,
        Suit::Honor => t - 27 + 1,
    }
}

/// Is this a terminal (1 or 9 in any suit)?
pub fn tile34_is_terminal(t: Tile34) -> bool {
    matches!(tile34_number(t), 1 | 9) && tile34_suit(t) != Suit::Honor
}

/// Is this an honor tile?
pub fn tile34_is_honor(t: Tile34) -> bool {
    t >= 27
}

/// Is this a terminal or honor?
pub fn tile34_is_yaochuu(t: Tile34) -> bool {
    tile34_is_terminal(t) || tile34_is_honor(t)
}

/// Is this a valid tile?
pub fn tile34_is_valid(t: Tile34) -> bool {
    t <= 33
}

/// Display a single tile34 as human-readable string
pub fn tile34_display(t: Tile34) -> &'static str {
    match t {
        0 => "1m", 1 => "2m", 2 => "3m", 3 => "4m", 4 => "5m", 5 => "6m", 6 => "7m", 7 => "8m", 8 => "9m",
        9 => "1p", 10 => "2p", 11 => "3p", 12 => "4p", 13 => "5p", 14 => "6p", 15 => "7p", 16 => "8p", 17 => "9p",
        18 => "1s", 19 => "2s", 20 => "3s", 21 => "4s", 22 => "5s", 23 => "6s", 24 => "7s", 25 => "8s", 26 => "9s",
        27 => "東", 28 => "南", 29 => "西", 30 => "北", 31 => "白", 32 => "發", 33 => "中",
        _ => "??",
    }
}

/// Display a list of tile34 as a sorted string (like "123m456p789s")
pub fn tiles34_display(tiles: &[Tile34]) -> String {
    let mut sorted = tiles.to_vec();
    sorted.sort();

    let mut result = String::new();
    for &suit in &[Suit::Man, Suit::Pin, Suit::Sou, Suit::Honor] {
        let numbers: Vec<u8> = sorted
            .iter()
            .filter(|&&t| tile34_suit(t) == suit)
            .map(|&t| tile34_number(t))
            .collect();
        if numbers.is_empty() {
            continue;
        }
        for n in numbers {
            result.push_str(&n.to_string());
        }
        result.push_str(match suit {
            Suit::Man => "m",
            Suit::Pin => "p",
            Suit::Sou => "s",
            Suit::Honor => "z",
        });
    }
    result
}

/// Display a single tile136
pub fn tile136_display(t: Tile136) -> String {
    let t34 = t136_to_t34(t);
    let base = tile34_display(t34);
    if is_red_five(t) {
        format!("[{}]", base)
    } else {
        base.to_string()
    }
}

/// Display a list of tile136
pub fn tiles136_display(tiles: &[Tile136]) -> String {
    let t34s: Vec<Tile34> = tiles.iter().map(|&t| t136_to_t34(t)).collect();
    tiles34_display(&t34s)
}

/// Constants for tile34
pub mod t34 {
    use super::Tile34;

    pub const M1: Tile34 = 0; pub const M2: Tile34 = 1; pub const M3: Tile34 = 2;
    pub const M4: Tile34 = 3; pub const M5: Tile34 = 4; pub const M6: Tile34 = 5;
    pub const M7: Tile34 = 6; pub const M8: Tile34 = 7; pub const M9: Tile34 = 8;

    pub const P1: Tile34 = 9; pub const P2: Tile34 = 10; pub const P3: Tile34 = 11;
    pub const P4: Tile34 = 12; pub const P5: Tile34 = 13; pub const P6: Tile34 = 14;
    pub const P7: Tile34 = 15; pub const P8: Tile34 = 16; pub const P9: Tile34 = 17;

    pub const S1: Tile34 = 18; pub const S2: Tile34 = 19; pub const S3: Tile34 = 20;
    pub const S4: Tile34 = 21; pub const S5: Tile34 = 22; pub const S6: Tile34 = 23;
    pub const S7: Tile34 = 24; pub const S8: Tile34 = 25; pub const S9: Tile34 = 26;

    pub const EAST: Tile34 = 27; pub const SOUTH: Tile34 = 28;
    pub const WEST: Tile34 = 29; pub const NORTH: Tile34 = 30;
    pub const HAKU: Tile34 = 31; pub const HATSU: Tile34 = 32; pub const CHUN: Tile34 = 33;

    /// All 34 tile types
    pub const ALL: [Tile34; 34] = [
        M1,M2,M3,M4,M5,M6,M7,M8,M9,
        P1,P2,P3,P4,P5,P6,P7,P8,P9,
        S1,S2,S3,S4,S5,S6,S7,S8,S9,
        EAST,SOUTH,WEST,NORTH,HAKU,HATSU,CHUN,
    ];

    /// Wind tiles (for seat/round wind)
    pub const WINDS: [Tile34; 4] = [EAST, SOUTH, WEST, NORTH];

    /// Dragon tiles
    pub const DRAGONS: [Tile34; 3] = [HAKU, HATSU, CHUN];
}

/// Constants for tile136 (Tenhou protocol format)
pub mod t136 {
    use super::Tile136;

    pub const M1_0: Tile136 = 0; pub const M1_1: Tile136 = 1; pub const M1_2: Tile136 = 2; pub const M1_3: Tile136 = 3;
    pub const M2_0: Tile136 = 4; pub const M2_1: Tile136 = 5; pub const M2_2: Tile136 = 6; pub const M2_3: Tile136 = 7;
    pub const M5R: Tile136 = 16;
    pub const P5R: Tile136 = 52;
    pub const S5R: Tile136 = 88;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversions() {
        assert_eq!(t136_to_t34(0), 0);   // 1m
        assert_eq!(t136_to_t34(16), 4);  // red 5m -> 5m
        assert_eq!(t136_to_t34(135), 33); // chun
        assert_eq!(t34_to_t136(0), 0);
        assert_eq!(t34_to_t136(5), 20);
    }

    #[test]
    fn test_red_five() {
        assert!(is_red_five(16));
        assert!(is_red_five(52));
        assert!(is_red_five(88));
        assert!(!is_red_five(15));
    }

    #[test]
    fn test_display() {
        assert_eq!(tile34_display(0), "1m");
        assert_eq!(tile34_display(33), "中");
        assert_eq!(tiles34_display(&[0, 1, 2, 9, 10, 11]), "123m123p");
    }
}
