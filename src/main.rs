//! Tenhou Auto-Play Bot
//!
//! An automatic mahjong AI bot for Tenhou.net.
//!
//! ## Quick Start
//!
//! 1. `cargo run --release -- --sim` — run local simulation
//! 2. Edit `config.toml` and set your user ID for live Tenhou play
//!
//! ## How to add your own strategy
//!
//! See `tactics.rs` — look for **CUSTOMIZE** comments to inject your understanding.

#![allow(dead_code)]

mod advisor;
mod ai;
mod client;
mod config;
mod meld;
mod protocol;
mod shanten;
mod sim;
mod state;
mod tactics;
mod tile;
mod yaku;

use crate::client::{BotConfig, TenhouBot};
use crate::config::BotSettings;
use crate::tactics::TacticsAi;

use clap::Parser;

#[derive(Parser)]
#[command(name = "tenhou-bot")]
#[command(about = "Automatic Tenhou mahjong AI bot")]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Tenhou user ID (overrides config)
    #[arg(short, long)]
    user_id: Option<String>,

    /// Lobby type (overrides config)
    #[arg(short, long)]
    lobby: Option<u8>,

    /// Game type (overrides config)
    #[arg(short, long)]
    game_type: Option<u8>,

    /// Run local simulation instead of connecting to Tenhou
    #[arg(long)]
    sim: bool,

    /// Number of games to simulate (with --sim)
    #[arg(long, default_value = "1")]
    games: u32,

    /// Advisor mode: read game state JSON from stdin, output best discard
    #[arg(long)]
    advise: bool,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        .format_timestamp_millis()
        .init();

    if cli.advise {
        crate::advisor::run_advisor()?;
        return Ok(());
    }

    log::info!("=== Tenhou Bot v{} ===", env!("CARGO_PKG_VERSION"));

    if cli.sim {
        // Local simulation mode — uses TacticsAi internally
        let _settings = load_config(&cli.config)?;
        crate::sim::run_simulation(cli.games);
        return Ok(());
    }

    // Live Tenhou mode
    let settings = load_config(&cli.config)?;
    let user_id = cli.user_id.clone().unwrap_or(settings.account.user_id.clone());
    let lobby = cli.lobby.unwrap_or(settings.game.lobby);
    let game_type = cli.game_type.unwrap_or(settings.game.game_type);

    log::info!("User ID: {}", user_id);
    log::info!("Lobby: {}, Game type: {}", lobby, game_type);

    let ai = build_ai(&settings);

    let bot_config = BotConfig {
        user_id, lobby, game_type,
        search_timeout: settings.game.search_timeout_secs,
    };

    let mut bot = TenhouBot::new(bot_config, ai);
    bot.run().await?;

    Ok(())
}

fn build_ai(settings: &BotSettings) -> TacticsAi {
    TacticsAi {
        allow_melds: settings.strategy.allow_melds,
        allow_riichi: true,
        fold_against_riichi: settings.strategy.fold_against_riichi,
        min_ukeire_for_riichi: settings.strategy.min_ukeire_for_riichi,
        ukeire_weight: settings.strategy.ukeire_weight,
        dora_weight: settings.strategy.dora_weight,
        danger_penalty: settings.strategy.danger_penalty,
        defense_weight: 0.0,
        min_shanten_improve_for_meld: settings.strategy.min_shanten_improve_for_meld,
    }
}

fn load_config(path: &str) -> anyhow::Result<BotSettings> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let settings: BotSettings = toml::from_str(&content)?;
            log::info!("Loaded config from {}", path);
            Ok(settings)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::warn!("Config file '{}' not found, using defaults", path);
            let default = BotSettings::default();
            let default_toml = toml::to_string_pretty(&default)?;
            std::fs::write(path, &default_toml)?;
            log::info!("Created default config at '{}'", path);
            Ok(default)
        }
        Err(e) => Err(e.into()),
    }
}
