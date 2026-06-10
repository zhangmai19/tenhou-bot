//! Game state tracking — maintains the full view of the game from the bot's perspective.

use crate::tile::{Tile136, Tile34, t136_to_t34, tile34_display, tiles136_display, tile34_suit, tile34_number};
use crate::meld::Meld;
use crate::protocol::{ServerMessage, TenhouMessage, parse_csv_i32, parse_csv_u16};

/// Player state from bot's perspective
#[derive(Debug, Clone)]
pub struct PlayerState {
    /// Player name
    pub name: String,
    /// Player level (dan/kyu)
    pub level: String,
    /// Current score
    pub score: i32,
    /// Tiles in hand (136 format), sorted. Empty for opponents.
    pub hand_136: Vec<Tile136>,
    /// Discard pile (136 format), in order
    pub discards: Vec<Tile136>,
    /// Called melds
    pub melds: Vec<Meld>,
    /// Whether this player has declared riichi
    pub is_riichi: bool,
    /// Seat position (0-3)
    pub seat: u8,
    /// Is this the bot?
    pub is_bot: bool,
}

impl PlayerState {
    pub fn new(seat: u8, is_bot: bool) -> Self {
        PlayerState {
            name: format!("Player{}", seat),
            level: String::new(),
            score: 25000,
            hand_136: Vec::new(),
            discards: Vec::new(),
            melds: Vec::new(),
            is_riichi: false,
            seat,
            is_bot,
        }
    }

    /// Get tiles in hand as tile34
    pub fn hand34(&self) -> Vec<Tile34> {
        self.hand_136.iter().map(|&t| t136_to_t34(t)).collect()
    }

    /// Get all tiles from melds as tile34
    pub fn meld_tiles34(&self) -> Vec<Tile34> {
        self.melds.iter().flat_map(|m| m.tiles34()).collect()
    }

    /// Count of tiles in hand
    pub fn hand_size(&self) -> usize {
        self.hand_136.len()
    }

    /// Player wind based on seat and dealer
    pub fn player_wind(&self, dealer_seat: u8) -> Tile34 {
        let relative = (4 + self.seat as i8 - dealer_seat as i8) as u8 % 4;
        match relative {
            0 => 27, // East
            1 => 28, // South
            2 => 29, // West
            3 => 30, // North
            _ => unreachable!(),
        }
    }
}

/// Overall game state
#[derive(Debug, Clone)]
pub struct GameState {
    /// All 4 players
    pub players: [PlayerState; 4],
    /// Bot's seat (always 0 in Tenhou protocol)
    pub bot_seat: u8,
    /// Dealer seat
    pub dealer_seat: u8,
    /// Round number (0=東1, 1=東2, ..., 3=東4, 4=南1, ...)
    pub round: u8,
    /// Honba count
    pub honba: u8,
    /// Reach sticks in pot
    pub reach_sticks: u8,
    /// Dora indicators (tiles that indicate what the dora is)
    pub dora_indicators: Vec<Tile136>,
    /// Whether red fives are in play
    pub aka_ari: bool,
    /// Whether open tanyao is allowed
    pub open_tanyao: bool,
    /// Whether this is a tonnansen (east+south) game
    pub is_tonnansen: bool,
    /// Round wind (27=東, 28=南, 29=西 based on round)
    pub round_wind: Tile34,
    /// Tiles remaining in the wall
    pub tiles_left: u8,
    /// Whether bot is currently in riichi
    pub bot_riichi: bool,
    /// Last drawn tile (for tsumogiri detection)
    pub last_drawn_tile: Option<Tile136>,
    /// Whose turn is it to discard (current actor)
    pub current_actor: Option<u8>,
    /// Is game in progress?
    pub game_active: bool,
    /// Lobby type
    pub lobby: u8,
    /// Game type (bitmask)
    pub game_type: u8,
    /// Is searching for a game
    pub is_searching: bool,
    /// Authenticated
    pub authenticated: bool,
}

impl GameState {
    pub fn new(lobby: u8, game_type: u8) -> Self {
        let players: [PlayerState; 4] = [
            PlayerState::new(0, true),
            PlayerState::new(1, false),
            PlayerState::new(2, false),
            PlayerState::new(3, false),
        ];

        GameState {
            players,
            bot_seat: 0,
            dealer_seat: 0,
            round: 0,
            honba: 0,
            reach_sticks: 0,
            dora_indicators: Vec::new(),
            aka_ari: true,
            open_tanyao: true,
            is_tonnansen: false,
            round_wind: 27, // East
            tiles_left: 70,
            bot_riichi: false,
            last_drawn_tile: None,
            current_actor: None,
            game_active: false,
            lobby,
            game_type,
            is_searching: false,
            authenticated: false,
        }
    }

    /// Get the bot player
    pub fn bot(&self) -> &PlayerState {
        &self.players[self.bot_seat as usize]
    }

    /// Get mutable bot player
    pub fn bot_mut(&mut self) -> &mut PlayerState {
        &mut self.players[self.bot_seat as usize]
    }

    /// Get an opponent by seat
    pub fn opponent(&self, seat: u8) -> &PlayerState {
        &self.players[seat as usize]
    }

    /// Get a player by seat
    pub fn player(&self, seat: u8) -> &PlayerState {
        &self.players[seat as usize]
    }

    /// Get the dora tiles (the actual bonus tiles, not the indicators)
    pub fn dora_tiles(&self) -> Vec<Tile34> {
        self.dora_indicators.iter().map(|&ind| indicator_to_dora(t136_to_t34(ind))).collect()
    }

    /// Update state from a Tenhou message
    pub fn update(&mut self, msg: &TenhouMessage) {
        match msg {
            TenhouMessage::Init { seed, ten, oya, hai, .. } => {
                self.apply_init(seed, ten, *oya, hai);
            }
            TenhouMessage::Reinit { seed, ten, oya, hai, kawa, melds } => {
                self.apply_reinit(seed, ten, *oya, hai, kawa, melds);
            }
            TenhouMessage::Draw { who, tile } => {
                self.tiles_left = self.tiles_left.saturating_sub(1);
                if *who == self.bot_seat {
                    if let Some(t) = tile {
                        self.bot_mut().hand_136.push(*t);
                        self.last_drawn_tile = Some(*t);
                    }
                }
                self.current_actor = Some(*who);
            }
            TenhouMessage::Discard { who, tile, is_tsumogiri: _ } => {
                if *who == self.bot_seat {
                    // Remove tile from bot's hand
                    self.bot_mut().hand_136.retain(|&t| t != *tile);
                    self.bot_mut().discards.push(*tile);
                } else {
                    self.players[*who as usize].discards.push(*tile);
                }
                self.last_drawn_tile = None;
                self.current_actor = None;
            }
            TenhouMessage::Meld { who, meld, .. } => {
                let who = *who;
                // Add meld to the caller
                self.players[who as usize].melds.push(meld.clone());
                // Note: consumed tiles are removed from hand later through the discard
                // or draw-then-discard flow
                self.current_actor = Some(who);
            }
            TenhouMessage::Reach { who, step } => {
                if *step == 1 {
                    self.players[*who as usize].is_riichi = true;
                    if *who == self.bot_seat {
                        self.bot_riichi = true;
                    }
                }
            }
            TenhouMessage::Dora { tile } => {
                self.dora_indicators.push(*tile);
            }
            TenhouMessage::Agari { .. } | TenhouMessage::Ryuukyoku { .. } => {
                self.game_active = false;
            }
            TenhouMessage::Go { game_type } => {
                self.game_active = true;
                self.apply_game_type(*game_type);
            }
            TenhouMessage::GameEnd { .. } => {
                self.game_active = false;
            }
            TenhouMessage::Un { names, levels } => {
                for i in 0..4 {
                    self.players[i as usize].name = names[i].clone();
                    self.players[i as usize].level = levels[i].clone();
                }
            }
            _ => {}
        }
    }

    /// Update state from a JSON protocol message (new format).
    pub fn update_json(&mut self, msg: &ServerMessage) {
        match msg {
            ServerMessage::Init { seed, ten, oya, hai } => {
                let seed_vals: Vec<i32> = seed.as_deref().map(parse_csv_i32).unwrap_or_default();
                let ten_vals: Vec<i32> = ten.as_deref().map(parse_csv_i32).unwrap_or_default();
                let oya_val: u8 = oya.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0);
                let hai_vals: Vec<Tile136> = hai.as_deref().map(parse_csv_u16).unwrap_or_default();
                self.apply_init(&seed_vals, &ten_vals, oya_val, &hai_vals);
            }
            ServerMessage::Un { n0, n1, n2, n3, .. } => {
                if let Some(ref name) = *n0 { self.players[0].name = url_decode(name); }
                if let Some(ref name) = *n1 { self.players[1].name = url_decode(name); }
                if let Some(ref name) = *n2 { self.players[2].name = url_decode(name); }
                if let Some(ref name) = *n3 { self.players[3].name = url_decode(name); }
            }
            ServerMessage::Go { game_type, .. } => {
                if let Some(gt) = game_type {
                    if let Ok(gt_int) = gt.parse::<i32>() {
                        self.apply_game_type(gt_int);
                    }
                }
            }
            ServerMessage::Dora { hai } => {
                if let Some(ref h) = *hai {
                    if let Ok(tile) = h.parse::<u16>() {
                        self.dora_indicators.push(tile as Tile136);
                    }
                }
            }
            _ => {}
        }
    }

    fn apply_init(&mut self, seed: &[i32], ten: &[i32], oya: u8, hai: &[Tile136]) {
        // seed format: [round, honba, reach_sticks, dora_indicator, ...]
        if seed.len() >= 1 {
            self.round = seed[0] as u8;
        }
        if seed.len() >= 2 {
            self.honba = seed[1] as u8;
        }
        if seed.len() >= 3 {
            self.reach_sticks = seed[2] as u8;
        }
        if seed.len() >= 4 {
            self.dora_indicators = seed[3..].iter().map(|&s| s as Tile136).collect();
        }

        // Update round wind
        self.round_wind = match self.round / 4 {
            0 => 27, // East
            1 => 28, // South
            _ => 29, // West (unlikely but possible)
        };

        // Scores
        if ten.len() == 4 {
            for i in 0..4 {
                self.players[i as usize].score = ten[i];
            }
        }

        self.dealer_seat = oya;

        // Reset players for new round
        for p in &mut self.players {
            p.hand_136.clear();
            p.discards.clear();
            p.melds.clear();
            p.is_riichi = false;
        }
        self.bot_riichi = false;

        // Set bot's initial hand
        self.bot_mut().hand_136 = hai.to_vec();
        self.bot_mut().hand_136.sort();
        self.tiles_left = 70;
        self.last_drawn_tile = None;
        self.current_actor = Some(oya);
    }

    fn apply_reinit(
        &mut self,
        seed: &[i32],
        ten: &[i32],
        oya: u8,
        hai: &[Tile136],
        kawa: &[Vec<Tile136>; 4],
        melds_list: &[Vec<crate::protocol::MeldInfo>; 4],
    ) {
        self.apply_init(seed, ten, oya, hai);

        for i in 0..4 {
            self.players[i as usize].discards = kawa[i].clone();
            // Check for reach (255 marker removed in parser)
            // melds
            for mi in &melds_list[i] {
                self.players[i as usize].melds.push(mi.meld.clone());
            }
        }
    }

    pub fn apply_game_type(&mut self, game_type: i32) {
        // Game type is an 8-bit bitmask
        let bits = format!("{:08b}", game_type);
        // bits[7] (MSB): 0=starter, 1=upper room, etc.
        // bits[6]: 1=fast game
        // bits[5]: 0=with red fives, 1=no red fives
        // bits[4]: 0=with open tanyao, 1=no open tanyao
        // bits[3]: 0=tonpuusen, 1=tonnansen
        // bits[2]: 0=four-player, 1=three-player
        let bytes: Vec<char> = bits.chars().collect();
        if bytes.len() >= 8 {
            self.aka_ari = bytes[5] == '0';
            self.open_tanyao = bytes[4] == '0';
            self.is_tonnansen = bytes[3] == '1';
        }
    }

    /// Visible tile counts (tiles we know are not available to draw)
    pub fn visible_counts(&self) -> [i8; 34] {
        let mut counts: [i8; 34] = [0; 34];

        // Bot's hand
        for &t in &self.bot().hand_136 {
            counts[t136_to_t34(t) as usize] += 1;
        }

        // Bot's melds
        for m in &self.bot().melds {
            for &t in &m.tiles {
                counts[t136_to_t34(t) as usize] += 1;
            }
        }

        // Dora indicators (face up)
        for &d in &self.dora_indicators {
            counts[t136_to_t34(d) as usize] += 1;
        }

        // Opponent discards
        for i in 0..4 {
            if i != self.bot_seat {
                for &t in &self.players[i as usize].discards {
                    counts[t136_to_t34(t) as usize] += 1;
                }
                // Opponent melds
                for m in &self.players[i as usize].melds {
                    for &t in &m.tiles {
                        counts[t136_to_t34(t) as usize] += 1;
                    }
                }
            }
        }

        counts
    }

    /// Display for logging
    pub fn display(&self) -> String {
        let bot = self.bot();
        format!(
            "Round: {}局{}本場 | Dora: {} | Hand: {} | Discards: {} | Melds: {} | Score: {}",
            self.round + 1,
            self.honba,
            self.dora_tiles().iter().map(|&t| tile34_display(t)).collect::<Vec<_>>().join(""),
            tiles136_display(&bot.hand_136),
            tiles136_display(&bot.discards),
            bot.melds.iter().map(|m| m.display()).collect::<Vec<_>>().join(", "),
            bot.score,
        )
    }
}

/// Simple URL decode (only handles %XX hex encoding)
fn url_decode(s: &str) -> String {
    let mut result = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(&s[i+1..i+3], 16) {
                result.push(hex as char);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            result.push(' ');
        } else {
            result.push(bytes[i] as char);
        }
        i += 1;
    }
    result
}

/// Convert a dora indicator to the actual dora tile.
/// For suits (0-26): dora = indicator + 1, wrapping at 9
/// For honors: special mapping
pub fn indicator_to_dora(indicator: Tile34) -> Tile34 {
    match indicator {
        // 1-8m → 2-9m, 9m → 1m
        0..=7 => indicator + 1,
        8 => 0,
        // 1-8p → 2-9p, 9p → 1p
        9..=16 => indicator + 1,
        17 => 9,
        // 1-8s → 2-9s, 9s → 1s
        18..=25 => indicator + 1,
        26 => 18,
        // 東→南→西→北→東
        27 => 28, // East → South
        28 => 29, // South → West
        29 => 30, // West → North
        30 => 27, // North → East
        // 白→發→中→白
        31 => 32, // Haku → Hatsu
        32 => 33, // Hatsu → Chun
        33 => 31, // Chun → Haku
        _ => indicator,
    }
}

/// Check if a discard is safe against a specific opponent (simplified version).
/// A tile is "safe" if:
/// - It's in the opponent's discard pile (genbutsu)
/// - It's a suji tile after their riichi
pub fn is_safe_tile(tile136: Tile136, opponent: &PlayerState, _all_discards: &[Tile136]) -> bool {
    let tile34 = t136_to_t34(tile136);

    // Genbutsu: exact same tile in opponent's discards
    if opponent.discards.iter().any(|&d| t136_to_t34(d) == tile34) {
        return true;
    }

    // If opponent is in riichi, check suji (same suit, ±3)
    // This is a simplification — full defense is more complex
    if opponent.is_riichi {
        let suit = tile34_suit(tile34);
        if suit != crate::tile::Suit::Honor {
            let num = tile34_number(tile34);
            // Suji: if 1-4-7, 2-5-8, 3-6-9 are all discarded
            for &suji_num in &[num, num + 3, num.wrapping_sub(3)] {
                if suji_num >= 1 && suji_num <= 9 {
                    // Check if opponent discarded any tile of that number in same suit
                    let suji_t34 = match suit {
                        crate::tile::Suit::Man => suji_num - 1,
                        crate::tile::Suit::Pin => 9 + suji_num - 1,
                        crate::tile::Suit::Sou => 18 + suji_num - 1,
                        _ => continue,
                    };
                    if opponent.discards.iter().any(|&d| t136_to_t34(d) == suji_t34) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indicator_to_dora() {
        assert_eq!(indicator_to_dora(0), 1);  // 1m → 2m
        assert_eq!(indicator_to_dora(8), 0);  // 9m → 1m
        assert_eq!(indicator_to_dora(27), 28); // 東 → 南
        assert_eq!(indicator_to_dora(33), 31); // 中 → 白
    }

    #[test]
    fn test_game_state_init() {
        let mut gs = GameState::new(0, 0);
        gs.update(&TenhouMessage::Init {
            seed: vec![0, 0, 0, 1, 2],
            ten: vec![25000, 25000, 25000, 25000],
            oya: 0,
            hai: vec![0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48],
            aka: true,
        });
        assert_eq!(gs.bot().hand_136.len(), 13);
        assert_eq!(gs.dealer_seat, 0);
        assert_eq!(gs.dora_indicators.len(), 2);
    }
}
