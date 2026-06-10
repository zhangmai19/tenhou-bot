//! Shanten number calculation.
//!
//! Uses recursive suit partitioning for optimal mentsu/taatsu counting.
//! Shanten = number of tiles away from tenpai.
//! - Shanten -1 = agari (complete hand)
//! - Shanten 0 = tenpai
//! - Shanten 1 = iishanten

use crate::tile::{Tile136, t136_to_t34};

const MENTSU_TARGET: i8 = 4;

/// Calculate normal shanten for a hand.
pub fn normal_shanten(counts: &[i8; 34]) -> i8 {
    let mut best = 99i8;

    // Try each possible head (pair)
    for i in 0..34 {
        if counts[i] >= 2 {
            let mut c = *counts;
            c[i] -= 2;
            let (k, t) = count_best_partition(&c);
            let s = 2 * (MENTSU_TARGET - k) - t - 1;
            best = best.min(s);
        }
    }

    // Try without fixed head
    {
        let (k, t) = count_best_partition(counts);
        let s = 2 * (MENTSU_TARGET + 1 - k) - t - 2;
        best = best.min(s);
    }

    best
}

/// Count best (k complete mentsu, t partial mentsu) from tiles.
fn count_best_partition(counts: &[i8; 34]) -> (i8, i8) {
    let man = best_suit_partition(&to_suit(counts, 0));
    let pin = best_suit_partition(&to_suit(counts, 9));
    let sou = best_suit_partition(&to_suit(counts, 18));

    let mut best_k = 0i8;
    let mut best_t = 0i8;

    for &(mk, mt) in &man {
        for &(pk, pt) in &pin {
            for &(sk, st) in &sou {
                let k = mk + pk + sk;
                let t = mt + pt + st;
                let (hk, ht) = honor_counts(counts);
                let k = k + hk;
                let t = t + ht;

                if k > best_k || (k == best_k && t > best_t) {
                    best_k = k;
                    best_t = t;
                }
            }
        }
    }

    (best_k, best_t)
}

fn to_suit(counts: &[i8; 34], start: usize) -> [i8; 9] {
    let mut suit = [0i8; 9];
    for i in 0..9 {
        suit[i] = counts[start + i];
    }
    suit
}

fn honor_counts(counts: &[i8; 34]) -> (i8, i8) {
    let mut k = 0i8;
    let mut t = 0i8;
    for i in 27..34 {
        match counts[i] {
            3 | 4 => k += 1,
            2 => t += 1,
            _ => {}
        }
    }
    (k, t)
}

/// All Pareto-optimal (k,t) partitions for a 9-tile suit.
fn best_suit_partition(suit: &[i8; 9]) -> Vec<(i8, i8)> {
    let mut results = Vec::new();
    suit_search(suit, 0, 0, 0, &mut results);

    // Sort descending by k then t, dedup
    results.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    results.dedup();

    // Keep only Pareto-optimal
    let mut optimal: Vec<(i8, i8)> = Vec::new();
    for &r in &results {
        if !optimal.iter().any(|o| o.0 >= r.0 && o.1 >= r.1) {
            optimal.push(r);
        }
    }
    if optimal.is_empty() {
        optimal.push((0, 0));
    }
    optimal
}

/// Recursive DFS for suit partition.
fn suit_search(suit: &[i8; 9], pos: usize, k: i8, t: i8, results: &mut Vec<(i8, i8)>) {
    if pos >= 9 {
        results.push((k, t));
        return;
    }
    if suit[pos] == 0 {
        suit_search(suit, pos + 1, k, t, results);
        return;
    }

    // 1. Koutsu
    if suit[pos] >= 3 {
        let mut s2 = *suit;
        s2[pos] -= 3;
        suit_search(&s2, pos, k + 1, t, results);
    }
    // 2. Shuntsu
    if pos + 2 < 9 && suit[pos] >= 1 && suit[pos + 1] >= 1 && suit[pos + 2] >= 1 {
        let mut s2 = *suit;
        s2[pos] -= 1;
        s2[pos + 1] -= 1;
        s2[pos + 2] -= 1;
        suit_search(&s2, pos, k + 1, t, results);
    }
    // 3. Pair (taatsu)
    if suit[pos] >= 2 {
        let mut s2 = *suit;
        s2[pos] -= 2;
        suit_search(&s2, pos + 1, k, t + 1, results);
    }
    // 4. Adjacent taatsu (ryanmen/kanchan)
    if pos + 1 < 9 && suit[pos] >= 1 && suit[pos + 1] >= 1 {
        let mut s2 = *suit;
        s2[pos] -= 1;
        s2[pos + 1] -= 1;
        suit_search(&s2, pos + 1, k, t + 1, results);
    }
    // 5. Gap taatsu (e.g. 4-6)
    if pos + 2 < 9 && suit[pos] >= 1 && suit[pos + 2] >= 1 {
        let mut s2 = *suit;
        s2[pos] -= 1;
        s2[pos + 2] -= 1;
        suit_search(&s2, pos + 1, k, t + 1, results);
    }
    // 6. Skip (isolated tile — no contribution)
    let mut s2 = *suit;
    s2[pos] = 0;
    suit_search(&s2, pos + 1, k, t, results);
}

/// Chiitoitsu shanten
pub fn chiitoitsu_shanten(counts: &[i8; 34]) -> i8 {
    let pairs = counts.iter().filter(|&&c| c >= 2).count() as i8;
    6 - pairs
}

/// Kokushi musou shanten
pub fn kokushi_shanten(counts: &[i8; 34]) -> i8 {
    let yaochuu: [usize; 13] = [0, 8, 9, 17, 18, 26, 27, 28, 29, 30, 31, 32, 33];
    let mut unique = 0i8;
    let mut has_pair = false;
    for &idx in &yaochuu {
        match counts[idx] {
            0 => {}
            1 => unique += 1,
            _ => { unique += 1; has_pair = true; }
        }
    }
    13 - unique - if has_pair { 1 } else { 0 }
}

/// Overall shanten
pub fn calc_shanten(hand_136: &[Tile136]) -> i8 {
    let mut counts: [i8; 34] = [0; 34];
    for &t in hand_136 {
        counts[t136_to_t34(t) as usize] += 1;
    }
    let normal = normal_shanten(&counts);
    let chiitoi = chiitoitsu_shanten(&counts);
    let kokushi = kokushi_shanten(&counts);
    normal.min(chiitoi).min(kokushi)
}

/// Detailed shanten with ukeire count
pub fn calc_shanten_detailed(hand_136: &[Tile136]) -> ShantenInfo {
    let mut counts: [i8; 34] = [0; 34];
    for &t in hand_136 {
        counts[t136_to_t34(t) as usize] += 1;
    }

    let normal = normal_shanten(&counts);
    let chiitoi = chiitoitsu_shanten(&counts);
    let kokushi = kokushi_shanten(&counts);

    let (shanten, hand_type) = if normal <= chiitoi && normal <= kokushi {
        (normal, HandType::Normal)
    } else if chiitoi <= kokushi {
        (chiitoi, HandType::Chiitoitsu)
    } else {
        (kokushi, HandType::Kokushi)
    };

    let mut ukeire_count = 0u32;
    for t34 in 0..34u8 {
        if counts[t34 as usize] >= 4 { continue; }
        let mut new_counts = counts;
        new_counts[t34 as usize] += 1;
        let new_shanten = match hand_type {
            HandType::Normal => normal_shanten(&new_counts),
            HandType::Chiitoitsu => chiitoitsu_shanten(&new_counts),
            HandType::Kokushi => kokushi_shanten(&new_counts),
        };
        if new_shanten < shanten {
            ukeire_count += (4 - counts[t34 as usize]) as u32;
        }
    }

    ShantenInfo { shanten, hand_type, ukeire_count }
}

#[derive(Debug, Clone, Copy)]
pub struct ShantenInfo {
    pub shanten: i8,
    pub hand_type: HandType,
    pub ukeire_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandType {
    Normal,
    Chiitoitsu,
    Kokushi,
}

impl ShantenInfo {
    pub fn is_tenpai(&self) -> bool { self.shanten <= 0 }
}

/// Best discard for shanten improvement
pub fn best_discard_for_shanten(hand_136: &[Tile136]) -> Option<(Tile136, i8, u32)> {
    let mut best: Option<(Tile136, i8, u32)> = None;
    for (i, &tile) in hand_136.iter().enumerate() {
        let mut new_hand: Vec<Tile136> = hand_136.to_vec();
        new_hand.remove(i);
        let info = calc_shanten_detailed(&new_hand);
        let is_better = match best {
            None => true,
            Some((_, bs, bu)) => info.shanten < bs || (info.shanten == bs && info.ukeire_count > bu),
        };
        if is_better {
            best = Some((tile, info.shanten, info.ukeire_count));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::Tile34;

    fn t136s(t34s: &[Tile34]) -> Vec<Tile136> {
        t34s.iter().map(|&t| (t as Tile136) * 4).collect()
    }

    #[test]
    fn test_tenpai_ryanmen() {
        // 13 tiles: 123m 456p 789s 東東 45m (ryanmen tenpai)
        let hand = t136s(&[0,1,2, 12,13,14, 24,25,26, 27,27, 3,4]);
        assert_eq!(calc_shanten(&hand), 0);
    }

    #[test]
    fn test_tenpai_tanki() {
        // 13 tiles: 123m 456p 789s 東東東 白 (tanki tenpai, wait 白)
        let hand = t136s(&[0,1,2, 12,13,14, 24,25,26, 27,27,27, 31]);
        assert_eq!(calc_shanten(&hand), 0);
    }

    #[test]
    fn test_iishanten() {
        // 13 tiles: 123m 456p 789s 東 白 4m 6m
        let hand = t136s(&[0,1,2, 12,13,14, 24,25,26, 27, 31, 3, 5]);
        assert_eq!(calc_shanten(&hand), 1);
    }

    #[test]
    fn test_complete_hand() {
        let mut counts = [0i8; 34];
        for &t in &[0,1,2, 12,13,14, 24,25,26, 27,27,27, 31,31] {
            counts[t as usize] += 1;
        }
        assert!(normal_shanten(&counts) < 0);
    }

    #[test]
    fn test_chiitoitsu_tenpai() {
        // 6 pairs (13 tiles) = tenpai
        let hand = t136s(&[0,0, 9,9, 18,18, 27,27, 31,31, 3,3, 5]);
        assert_eq!(calc_shanten(&hand), 0);
    }

    #[test]
    fn test_kokushi_tenpai() {
        // 13 tiles, all 13 types = tenpai (waiting for pair)
        let hand = t136s(&[0,8,9,17,18,26,27,28,29,30,31,32,33]);
        assert_eq!(calc_shanten(&hand), 0);
    }

    #[test]
    fn test_detailed_ukeire() {
        let hand = t136s(&[0,1,2, 12,13,14, 24,25,26, 27,27, 3,4]);
        let info = calc_shanten_detailed(&hand);
        assert_eq!(info.shanten, 0);
        assert!(info.ukeire_count > 0);
    }
}
