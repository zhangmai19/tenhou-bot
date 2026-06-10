//! Yaku (役) detection for determining if a hand can win.
//!
//! A winning hand must have at least one yaku. This module checks the
//! most common yaku to support basic gameplay.

use crate::tile::{Tile34, Tile136, t136_to_t34, tile34_number};

/// Information about the hand needed for yaku checking.
pub struct WinContext {
    /// All tiles in the winning hand (hand + melds + winning tile, as tile34)
    pub hand_tiles: [i8; 34],
    /// Whether the hand is closed (menzen) — no chi/pon/daiminkan
    pub menzen: bool,
    /// Whether riichi was declared
    pub riichi: bool,
    /// Whether this is a tsumo win (vs ron)
    pub tsumo: bool,
    /// Round wind (tile34: 27=E, 28=S, 29=W)
    pub round_wind: Tile34,
    /// Seat wind (tile34: 27=E, 28=S, 29=W, 30=N)
    pub seat_wind: Tile34,
}

/// Result of yaku checking.
pub struct YakuResult {
    /// Whether the hand has at least one valid yaku
    pub can_win: bool,
    /// List of yaku names found
    pub yaku_list: Vec<&'static str>,
}

/// Check if a hand has any yaku. This is the main entry point.
pub fn has_yaku(ctx: &WinContext) -> YakuResult {
    let mut yaku = Vec::new();

    // 1. Riichi — always valid if declared and menzen
    if ctx.riichi && ctx.menzen {
        yaku.push("立直");
    }

    // 2. Menzen tsumo — closed hand, self-draw
    if ctx.menzen && ctx.tsumo {
        yaku.push("門前清自摸和");
    }

    // 3. Tanyao — no terminals or honors
    if is_tanyao(&ctx.hand_tiles) {
        yaku.push("断么九");
    }

    // 4. Yakuhai — triplet of value tiles
    for &(tile, name) in &[
        (27, "場風"), (28, "場風"), (29, "場風"), (30, "場風"),
        (31, "白"), (32, "發"), (33, "中"),
    ] {
        if tile == ctx.round_wind && ctx.hand_tiles[tile as usize] >= 3 {
            if !yaku.contains(&"場風") && tile == ctx.round_wind {
                yaku.push(if ctx.round_wind == 27 { "場風(東)" }
                    else if ctx.round_wind == 28 { "場風(南)" }
                    else { "場風" });
            }
        }
        if tile == ctx.seat_wind && ctx.hand_tiles[tile as usize] >= 3 {
            yaku.push(if ctx.seat_wind == 27 { "自風(東)" }
                else if ctx.seat_wind == 28 { "自風(南)" }
                else if ctx.seat_wind == 29 { "自風(西)" }
                else { "自風(北)" });
        }
        if tile >= 31 && ctx.hand_tiles[tile as usize] >= 3 {
            yaku.push(name);
        }
    }
    // Dedup (manual simple approach)
    yaku.sort();
    yaku.dedup();

    // 5. Pinfu — not implemented yet (requires detailed hand parsing)

    // 6. Iipeikou, sanshoku, etc. — not implemented

    YakuResult {
        can_win: !yaku.is_empty(),
        yaku_list: yaku,
    }
}

/// Check if a hand is tanyao (no terminals or honors).
fn is_tanyao(counts: &[i8; 34]) -> bool {
    // Check that no yaochuu tile appears
    for i in 0..34 {
        if counts[i] > 0 && is_yaochuu(i as Tile34) {
            return false;
        }
    }
    true
}

fn is_yaochuu(t: Tile34) -> bool {
    if t >= 27 { return true; } // honors
    let n = tile34_number(t);
    n == 1 || n == 9
}

/// Convenience: check if the bot can win based on game state and winning tile.
/// Returns the yaku result.
pub fn can_bot_win(
    hand_136: &[Tile136],
    melds: &[crate::meld::Meld],
    win_tile: Tile136,
    menzen: bool,
    riichi: bool,
    tsumo: bool,
    round_wind: Tile34,
    seat_wind: Tile34,
) -> YakuResult {
    let mut counts: [i8; 34] = [0; 34];
    for &t in hand_136 {
        counts[t136_to_t34(t) as usize] += 1;
    }
    for m in melds {
        for &t in &m.tiles {
            counts[t136_to_t34(t) as usize] += 1;
        }
    }
    // Add winning tile
    counts[t136_to_t34(win_tile) as usize] += 1;

    // If riichi, the hand is closed
    let ctx = WinContext {
        hand_tiles: counts,
        menzen: menzen || riichi,
        riichi,
        tsumo,
        round_wind,
        seat_wind,
    };

    has_yaku(&ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_counts(tiles: &[Tile34]) -> [i8; 34] {
        let mut c = [0i8; 34];
        for &t in tiles { c[t as usize] += 1; }
        c
    }

    #[test]
    fn test_riichi_yaku() {
        let ctx = WinContext {
            hand_tiles: make_counts(&[0,1,2, 12,13,14, 24,25,26, 27,27,27, 31,31, 3]),
            menzen: true,
            riichi: true,
            tsumo: true,
            round_wind: 27,
            seat_wind: 28,
        };
        let r = has_yaku(&ctx);
        assert!(r.can_win);
        assert!(r.yaku_list.contains(&"立直"));
    }

    #[test]
    fn test_menzen_tsumo() {
        let ctx = WinContext {
            hand_tiles: make_counts(&[0,1,2, 12,13,14, 24,25,26, 27,27,27, 31,31, 3]),
            menzen: true,
            riichi: false,
            tsumo: true,
            round_wind: 27,
            seat_wind: 28,
        };
        let r = has_yaku(&ctx);
        assert!(r.can_win);
        assert!(r.yaku_list.contains(&"門前清自摸和"));
    }

    #[test]
    fn test_tanyao() {
        let ctx = WinContext {
            hand_tiles: make_counts(&[3,4,5, 12,13,14, 21,22,23, 3,3,3, 13,13,14]),
            menzen: false,
            riichi: false,
            tsumo: false,
            round_wind: 27,
            seat_wind: 28,
        };
        let r = has_yaku(&ctx);
        assert!(r.can_win);
        assert!(r.yaku_list.contains(&"断么九"));
    }

    #[test]
    fn test_no_yaku() {
        // Open hand with no tanyao, no yakuhai, no riichi = can't win
        let ctx = WinContext {
            hand_tiles: make_counts(&[0,1,2, 12,13,14, 24,25,26, 27,27, 31,31, 0,0,0]),
            menzen: false,
            riichi: false,
            tsumo: false,
            round_wind: 28,
            seat_wind: 29,
        };
        let r = has_yaku(&ctx);
        assert!(!r.can_win);
    }

    #[test]
    fn test_yakuhai_haku() {
        let ctx = WinContext {
            hand_tiles: make_counts(&[0,1,2, 12,13,14, 24,25,26, 31,31,31, 27,27, 0]),
            menzen: false,
            riichi: false,
            tsumo: false,
            round_wind: 27,
            seat_wind: 28,
        };
        let r = has_yaku(&ctx);
        assert!(r.can_win);
        assert!(r.yaku_list.contains(&"白"));
    }
}
