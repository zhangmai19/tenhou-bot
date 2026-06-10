//! Configuration loading for the Tenhou bot.

use serde::{Deserialize, Serialize};

/// Top-level configuration loaded from config.toml
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BotSettings {
    /// Tenhou account
    pub account: AccountConfig,
    /// Game preferences
    pub game: GameConfig,
    /// AI strategy settings
    pub strategy: StrategyConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AccountConfig {
    /// Tenhou user ID (registered account)
    pub user_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GameConfig {
    /// Lobby type:
    /// 0 = 一般 (general)
    /// 1 = 上級 (upper)
    /// 2 = 特上 (superior)
    /// 3 = 鳳凰 (phoenix)
    pub lobby: u8,
    /// Game type bitmask:
    /// - 0: Tonpuusen (east only), aka-ari, open tanyao allowed
    /// - 1: Tonnansen (east+south), aka-ari, open tanyao allowed
    pub game_type: u8,
    /// How many games to play before stopping (0 = unlimited)
    pub max_games: u32,
    /// Seconds to wait for matchmaking before retrying (0 = wait forever)
    pub search_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StrategyConfig {
    /// Minimum ukeire (tile acceptance count) for declaring riichi
    pub min_ukeire_for_riichi: u32,
    /// Whether to allow calling melds (chi/pon)
    pub allow_melds: bool,
    /// Whether to fold (play defensive) when an opponent riichis
    pub fold_against_riichi: bool,
    /// Weight for ukeire in discard evaluation (higher = more aggressive)
    pub ukeire_weight: f64,
    /// Weight for keeping dora (higher = value dora more)
    pub dora_weight: f64,
    /// Penalty for dangerous discards (higher = more defensive)
    pub danger_penalty: f64,
    /// Minimum shanten improvement required to call a meld
    pub min_shanten_improve_for_meld: i8,
}

impl Default for BotSettings {
    fn default() -> Self {
        BotSettings {
            account: AccountConfig {
                user_id: "NoName".to_string(),
            },
            game: GameConfig {
                lobby: 0,
                game_type: 0,
                max_games: 0,
                search_timeout_secs: 120,
            },
            strategy: StrategyConfig {
                min_ukeire_for_riichi: 4,
                allow_melds: true,
                fold_against_riichi: true,
                ukeire_weight: 1.0,
                dora_weight: 0.5,
                danger_penalty: 3.0,
                min_shanten_improve_for_meld: 2,
            },
        }
    }
}
