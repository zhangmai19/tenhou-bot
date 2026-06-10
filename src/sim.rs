//! Local mahjong game simulator for testing the bot's AI.
//!
//! Simulates a complete hanchan with:
//! - Full wall (136 tiles including red fives)
//! - Riichi, tsumo, ron, ryuukyoku
//! - Basic scoring (tsumo/ron payments, riichi sticks, honba)
//! - Bot uses TacticsAi, opponents auto-riichi when tenpai

use crate::ai::AiStrategy;
use crate::meld::MeldType;
use crate::shanten;
use crate::tactics::TacticsAi;
use crate::tile::{Tile136, Tile34, t136_to_t34, tile136_display, tiles136_display};
use crate::yaku::{self, WinContext};

use rand::seq::SliceRandom;
use rand::thread_rng;

// ── Opponent AI ─────────────────────────────────────────────────────────

struct SimpleAi { fold_against_riichi: bool }

impl SimpleAi {
    fn new() -> Self { SimpleAi { fold_against_riichi: true } }

    fn choose_discard(&self, hand: &[Tile136], riichi: bool) -> Tile136 {
        if riichi { return *hand.last().unwrap(); } // tsumogiri
        let mut best = hand[0];
        let mut best_s = 99i8;
        let mut best_u = 0u32;
        for (i, &t) in hand.iter().enumerate() {
            let mut h2 = hand.to_vec(); h2.remove(i);
            let info = shanten::calc_shanten_detailed(&h2);
            if info.shanten < best_s || (info.shanten == best_s && info.ukeire_count > best_u) {
                best_s = info.shanten; best_u = info.ukeire_count; best = t;
            }
        }
        best
    }
}

// ── Player ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct SimPlayer {
    hand: Vec<Tile136>,
    discards: Vec<Tile136>,
    melds: Vec<SimMeld>,
    riichi: bool,
    score: i32,
    seat: usize,
}

#[derive(Debug, Clone)]
struct SimMeld {
    mtype: MeldType,
    tiles: Vec<Tile136>,
    called_tile: Tile136,
    from: usize,
}

impl SimPlayer {
    fn new(seat: usize) -> Self {
        SimPlayer { hand: vec![], discards: vec![], melds: vec![], riichi: false, score: 25000, seat }
    }
}

// ── Simulator ────────────────────────────────────────────────────────────

pub struct Simulator {
    players: [SimPlayer; 4],
    wall: Vec<Tile136>,
    wall_pos: usize,
    dead_wall_start: usize,
    dora_indicators: Vec<Tile136>,
    round: u8,
    honba: u8,
    dealer: usize,
    round_wind: Tile34,
    bot_ai: TacticsAi,
    pub log: Vec<String>,
    /// Accumulated riichi sticks
    riichi_sticks: i32,
}

impl Simulator {
    pub fn new(bot_ai: TacticsAi) -> Self {
        Simulator {
            players: [SimPlayer::new(0), SimPlayer::new(1), SimPlayer::new(2), SimPlayer::new(3)],
            wall: vec![], wall_pos: 0, dead_wall_start: 0,
            dora_indicators: vec![],
            round: 0, honba: 0, dealer: 0, round_wind: 27,
            bot_ai, log: vec![], riichi_sticks: 0,
        }
    }

    pub fn run_hanchan(&mut self) -> [i32; 4] {
        for p in &mut self.players { p.score = 25000; }
        self.dealer = 0;
        self.riichi_sticks = 0;

        for r in 0..8 {
            self.round = r;
            self.round_wind = if r < 4 { 27 } else { 28 };
            self.log.push(format!("── {}局 {}本場 ──", round_name(r), self.honba));
            self.run_round();
        }

        // Final uma (placement bonus): 1st +20, 2nd +10, 3rd -10, 4th -20
        let mut scores: Vec<(usize, i32)> = (0..4).map(|i| (i, self.players[i].score)).collect();
        scores.sort_by_key(|s| -s.1);
        let uma = [20, 10, -10, -20];
        for (rank, &(pid, _)) in scores.iter().enumerate() {
            self.players[pid].score += uma[rank] * 100; // convert to actual points
        }

        self.log.push("═══ Game Over ═══".to_string());
        for i in 0..4 {
            self.log.push(format!("P{}: {} pts", i, self.players[i].score));
        }
        [self.players[0].score, self.players[1].score, self.players[2].score, self.players[3].score]
    }

    fn run_round(&mut self) {
        self.build_wall();
        self.wall_pos = 0;
        self.dora_indicators.clear();

        for p in &mut self.players {
            p.hand.clear(); p.discards.clear(); p.melds.clear(); p.riichi = false;
        }

        // Deal
        for i in 0..4 {
            let pid = (self.dealer + i) % 4;
            for _ in 0..13 {
                let t = self.draw_wall();
                self.players[pid].hand.push(t);
            }
            self.players[pid].hand.sort();
        }

        // Initial dora
        self.dora_indicators.push(self.wall[self.dead_wall_start]);
        self.log.push(format!("Dora: {}", tile136_display(self.dora_indicators[0])));

        let mut turn = self.dealer;

        for _ in 0..300 {
            if self.wall_pos >= self.dead_wall_start - 14 {
                self.handle_ryuukyoku();
                return;
            }

            let tile = self.draw_wall();
            self.players[turn].hand.push(tile);
            let is_riichi = self.players[turn].riichi;
            let current_hand = self.players[turn].hand.clone();

            // Tsumo check
            if shanten::calc_shanten(&current_hand) < 0 {
                if turn == 0 {
                    let ctx = self.make_win_ctx(0, tile, true);
                    if yaku::has_yaku(&ctx).can_win || is_riichi {
                        self.log.push(format!("🎉 BOT TSUMO! {}", tiles136_display(&current_hand)));
                        self.apply_tsumo(turn); return;
                    }
                } else if is_riichi {
                    self.log.push(format!("P{} TSUMO!", turn));
                    self.apply_tsumo(turn); return;
                }
            }

            // Riichi check
            let do_riichi = if !is_riichi {
                let menzen = self.players[turn].melds.iter().all(|m| matches!(m.mtype, MeldType::Ankan));
                menzen && shanten::calc_shanten(&current_hand) <= 0
            } else { false };

            if do_riichi {
                if turn == 0 {
                    let (dr, _) = self.bot_ai.should_riichi(&self.bot_state());
                    if dr {
                        self.players[0].riichi = true;
                        self.riichi_sticks += 1;
                        self.players[0].score -= 1000;
                        self.log.push("BOT declares RIICHI!".to_string());
                    }
                } else {
                    self.players[turn].riichi = true;
                    self.riichi_sticks += 1;
                    self.players[turn].score -= 1000;
                    self.log.push(format!("P{} declares RIICHI!", turn));
                }
            }

            // Choose discard
            let discard = if turn == 0 {
                let d = self.bot_ai.choose_discard(&self.bot_state());
                self.players[0].hand.retain(|&t| t != d);
                d
            } else {
                let ai = SimpleAi::new();
                let d = ai.choose_discard(&current_hand, self.players[turn].riichi);
                self.players[turn].hand.retain(|&t| t != d);
                d
            };
            self.players[turn].discards.push(discard);

            if turn == 0 {
                let rm = if self.players[0].riichi { "(R)" } else { "" };
                self.log.push(format!("BOT{} discards {} | hand: {} | shanten: {}",
                    rm, tile136_display(discard), tiles136_display(&self.players[0].hand),
                    shanten::calc_shanten(&self.players[0].hand)));
            }

            if let Some(winner) = self.check_ron(turn, discard) {
                self.apply_ron(winner, turn);
                return;
            }

            turn = (turn + 1) % 4;
        }
    }

    fn apply_tsumo(&mut self, winner: usize) {
        let base = if winner == self.dealer { 400 } else { 300 };
        let payment = base + self.honba as i32 * 100;
        let riichi_bonus = self.riichi_sticks * 1000;
        self.riichi_sticks = 0;

        if winner == self.dealer {
            // Dealer tsumo: each non-dealer pays equally
            let per = payment / 3;
            for i in 0..4 {
                if i != winner { self.players[i].score -= per; }
            }
            self.players[winner].score += payment + riichi_bonus;
        } else {
            // Non-dealer tsumo: dealer pays double
            let dealer_pay = (payment * 2) / 4;
            let other_pay = payment / 4;
            for i in 0..4 {
                if i != winner {
                    let amt = if i == self.dealer { dealer_pay } else { other_pay };
                    self.players[i].score -= amt;
                }
            }
            self.players[winner].score += dealer_pay + 2 * other_pay + riichi_bonus;
        }
        self.log.push(format!("Tsumo! Winner P{} | scores: {:?}", winner,
            self.players.iter().map(|p| p.score).collect::<Vec<_>>()));
    }

    fn apply_ron(&mut self, winner: usize, discarder: usize) {
        let base = if winner == self.dealer { 500 } else { 350 };
        let payment = base + self.honba as i32 * 100;
        let riichi_bonus = self.riichi_sticks * 1000;
        self.riichi_sticks = 0;

        self.players[discarder].score -= payment;
        self.players[winner].score += payment + riichi_bonus;

        self.log.push(format!("Ron! P{} wins from P{} (+{}) | scores: {:?}",
            winner, discarder, payment,
            self.players.iter().map(|p| p.score).collect::<Vec<_>>()));
    }

    fn handle_ryuukyoku(&mut self) {
        let mut tenpai = vec![false; 4];
        for i in 0..4 {
            tenpai[i] = shanten::calc_shanten(&self.players[i].hand) <= 0;
        }
        let count = tenpai.iter().filter(|&&t| t).count();
        self.log.push(format!("Ryuukyoku. Tenpai: {:?}", tenpai.iter().enumerate()
            .filter(|(_, &t)| t).map(|(i, _)| i).collect::<Vec<_>>()));

        if count == 0 || count == 4 {
            // No exchange
        } else {
            // Tenpai players split the noten payment (3000 per noten player)
            let per_noten: i32 = 1000;
            let per_tenpai: i32 = (per_noten * (4 - count) as i32) / count as i32;
            for i in 0..4 {
                if tenpai[i] { self.players[i].score += per_tenpai; }
                else { self.players[i].score -= per_noten; }
            }
        }

        // Return riichi sticks to riichi players (simplified: just add back)
        // Actually in real mahjong, riichi sticks carry over to next round.
        // Simplified: return them to the riichi players
        for i in 0..4 {
            if self.players[i].riichi {
                self.players[i].score += 1000;
                self.riichi_sticks -= 1i32;
            }
        }

        // Dealer tenpai = renchan
        if tenpai[self.dealer] {
            self.honba += 1;
        } else {
            self.dealer = (self.dealer + 1) % 4;
            self.honba = 0;
        }
    }

    fn check_ron(&self, discarder: usize, tile: Tile136) -> Option<usize> {
        for pid in 0..4 {
            if pid == discarder { continue; }
            let mut hand = self.players[pid].hand.clone();
            hand.push(tile);
            if shanten::calc_shanten(&hand) < 0 {
                if pid == 0 {
                    let ctx = self.make_win_ctx(0, tile, false);
                    if yaku::has_yaku(&ctx).can_win || self.players[0].riichi { return Some(0); }
                } else if self.players[pid].riichi {
                    return Some(pid);
                }
            }
        }
        None
    }

    fn draw_wall(&mut self) -> Tile136 { let t = self.wall[self.wall_pos]; self.wall_pos += 1; t }

    fn build_wall(&mut self) {
        let mut tiles = Vec::with_capacity(136);
        for t34 in 0..34u8 {
            for copy in 0..4 {
                let t136 = (t34 as Tile136) * 4 + (copy as Tile136);
                if t34 == 4 && copy == 0 { tiles.push(16); }
                else if t34 == 13 && copy == 0 { tiles.push(52); }
                else if t34 == 22 && copy == 0 { tiles.push(88); }
                else { tiles.push(t136); }
            }
        }
        tiles.shuffle(&mut thread_rng());
        self.wall = tiles;
        self.dead_wall_start = 122;
    }

    fn bot_state(&self) -> crate::state::GameState {
        let mut gs = crate::state::GameState::new(0, 0);
        gs.bot_mut().hand_136 = self.players[0].hand.clone();
        gs.bot_riichi = self.players[0].riichi;
        gs.bot_mut().is_riichi = self.players[0].riichi;
        gs.dealer_seat = self.dealer as u8;
        gs.round_wind = self.round_wind;
        for &dora in &self.dora_indicators { gs.dora_indicators.push(dora); }
        for i in 1..4 {
            gs.players[i].discards = self.players[i].discards.clone();
            gs.players[i].is_riichi = self.players[i].riichi;
        }
        gs
    }

    fn make_win_ctx(&self, pid: usize, win_tile: Tile136, tsumo: bool) -> WinContext {
        let p = &self.players[pid];
        let mut counts = [0i8; 34];
        for &t in &p.hand { counts[t136_to_t34(t) as usize] += 1; }
        for m in &p.melds { for &t in &m.tiles { counts[t136_to_t34(t) as usize] += 1; } }
        counts[t136_to_t34(win_tile) as usize] += 1;

        let menzen = p.melds.iter().all(|m| matches!(m.mtype, MeldType::Ankan));
        let seat_wind = match (4 + pid as i8 - self.dealer as i8) % 4 {
            0 => 27, 1 => 28, 2 => 29, _ => 30,
        };
        WinContext { hand_tiles: counts, menzen, riichi: p.riichi, tsumo, round_wind: self.round_wind, seat_wind }
    }
}

fn round_name(r: u8) -> String {
    format!("{}{}局", if r < 4 { "東" } else { "南" }, (r % 4) + 1)
}

pub fn run_simulation(games: u32) {
    println!("=== Tenhou Bot Simulator ===\n");
    println!("Running {} hanchan(s)...\n", games);

    let mut totals = [0i32; 4];
    let mut bot_wins = 0u32;

    for g in 1..=games {
        println!("── Game {} ──", g);
        let mut sim = Simulator::new(TacticsAi::default());
        let scores = sim.run_hanchan();

        for line in &sim.log { println!("  {}", line); }

        for i in 0..4 { totals[i] += scores[i]; }
        if scores[0] >= scores[1] && scores[0] >= scores[2] && scores[0] >= scores[3] {
            bot_wins += 1;
        }
        println!("  Final: Bot={} P1={} P2={} P3={}\n", scores[0], scores[1], scores[2], scores[3]);
    }

    println!("═══ Summary ({} games) ═══", games);
    println!("Bot: avg {} pts | P1: {} | P2: {} | P3: {}",
        totals[0]/games as i32, totals[1]/games as i32, totals[2]/games as i32, totals[3]/games as i32);
    println!("Bot 1st place: {}/{} games", bot_wins, games);
}
