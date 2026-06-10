//! WebSocket client for Tenhou HTML5 (mjv.jp).
//!
//! Protocol: JSON messages over WebSocket text frames.
//! Flow: HELO → LN → JOIN → REJOIN → JOIN(r) → GO → GOK → NEXTREADY → [play]

use crate::ai::AiStrategy;
use crate::protocol::{self, ServerMessage};
use crate::state::GameState;
use crate::tactics::TacticsAi;

use futures_util::{SinkExt, StreamExt};
use tokio::time::{self, Duration};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::client::generate_key;

pub struct BotConfig {
    pub user_id: String,
    pub lobby: u8,
    pub game_type: u8,
    pub search_timeout: u64,
}

impl Default for BotConfig {
    fn default() -> Self {
        BotConfig { user_id: "NoName".to_string(), lobby: 0, game_type: 0, search_timeout: 120 }
    }
}

pub struct TenhouBot<A: AiStrategy = TacticsAi> {
    config: BotConfig,
    ai: A,
    state: GameState,
}

impl<A: AiStrategy> TenhouBot<A> {
    pub fn new(config: BotConfig, ai: A) -> Self {
        TenhouBot { state: GameState::new(config.lobby, config.game_type), config, ai }
    }

    /// Connect and play. Uses tokio async runtime.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        let url = "wss://b-ww.mjv.jp/";
        log::info!("Connecting to {}...", url);

        // Build request with all WebSocket headers + Origin
        let uri: tokio_tungstenite::tungstenite::http::Uri = url.parse()?;
        let req = tokio_tungstenite::tungstenite::http::Request::builder()
            .uri(&uri)
            .header("Origin", "https://tenhou.net")
            .header("Host", "b-ww.mjv.jp")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", generate_key())
            .body(())?;

        let (ws_stream, _resp) = tokio_tungstenite::connect_async(req).await?;
        log::info!("WebSocket connected!");

        let (mut write, mut read) = ws_stream.split();

        // 1. HELO
        let helo = protocol::build_helo(&self.config.user_id);
        log::info!("> HELO");
        write.send(Message::Text(helo.into())).await?;

        // Read HELO response + LN
        loop {
            let msg = self.read_msg(&mut read).await?;
            log::debug!("< {}", msg);
            let parsed: ServerMessage = match serde_json::from_str(&msg) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let mut got_ln = false;
            match &parsed {
                ServerMessage::Helo { .. } => {
                    log::info!("Got HELO response");
                }
                ServerMessage::Ln => {
                    log::info!("Got LN — lobby ready");
                    got_ln = true;
                }
                _ => {
                    self.state.update_json(&parsed);
                }
            }
            if got_ln { break; }
        }

        // 2. Join lobby
        let join = protocol::build_join(self.config.lobby, self.config.game_type);
        log::info!("> JOIN");
        write.send(Message::Text(join.into())).await?;
        self.state.is_searching = true;

        // 3. Main game loop
        let mut last_keepalive = tokio::time::Instant::now();
        let mut pending_messages: Vec<String> = Vec::new();

        loop {
            // Keepalive every 15s
            if last_keepalive.elapsed() > Duration::from_secs(15) {
                write.send(Message::Text(protocol::build_keepalive().into())).await?;
                last_keepalive = tokio::time::Instant::now();
            }

            // Send any pending responses
            for msg in pending_messages.drain(..) {
                log::debug!("> {}", msg);
                write.send(Message::Text(msg.into())).await?;
            }

            // Read with timeout
            let msg_str = match time::timeout(Duration::from_secs(2), self.read_msg(&mut read)).await {
                Ok(Ok(m)) => m,
                Ok(Err(e)) => { log::error!("Read error: {}", e); break; }
                Err(_timeout) => continue, // timeout is ok
            };

            log::debug!("< {}", msg_str);

            let parsed: ServerMessage = match serde_json::from_str(&msg_str) {
                Ok(p) => p,
                Err(e) => {
                    log::warn!("Failed to parse: {} — {}", msg_str, e);
                    continue;
                }
            };

            // Handle special messages inline
            match &parsed {
                ServerMessage::Rejoin { .. } => {
                    log::info!("Rejoin requested");
                    let rejoin = protocol::build_join(self.config.lobby, self.config.game_type);
                    pending_messages.push(rejoin);
                    continue;
                }
                ServerMessage::Go { .. } => {
                    log::info!("Game found!");
                    self.state.is_searching = false;
                    self.state.game_active = true;
                    pending_messages.push(protocol::build_gok());
                    pending_messages.push(protocol::build_nextready());
                    self.state.update_json(&parsed);
                    continue;
                }
                ServerMessage::Taikyoku { .. } => {
                    log::info!("対局開始!");
                    self.state.update_json(&parsed);
                    // Server will send INIT next
                    continue;
                }
                ServerMessage::Prof => {
                    log::info!("Game ended (PROF)");
                    self.state.game_active = false;
                    continue;
                }
                _ => {}
            }

            self.state.update_json(&parsed);

            // Process game messages and generate responses
            let responses = self.process_message(&msg_str, &parsed)?;
            pending_messages.extend(responses);
        }

        Ok(())
    }

    async fn read_msg(&mut self, read: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin)) -> anyhow::Result<String> {
        loop {
            match read.next().await {
                Some(Ok(Message::Text(t))) => return Ok(t.to_string()),
                Some(Ok(Message::Ping(_))) => continue,
                Some(Ok(Message::Close(_))) => anyhow::bail!("Connection closed"),
                Some(Err(e)) => return Err(e.into()),
                None => anyhow::bail!("Stream ended"),
                _ => continue,
            }
        }
    }

    fn process_message(&mut self, raw: &str, parsed: &ServerMessage) -> anyhow::Result<Vec<String>> {
        let mut out = Vec::new();

        match parsed {
            ServerMessage::Init { seed, ten, oya, hai } => {
                log::info!("=== New Round ===");
                log::info!("Seed: {:?} Ten: {:?} Oya: {:?}", seed, ten, oya);
                if let Some(h) = hai {
                    log::info!("Hand: {:?}", h);
                }
                // Bot may be dealer, wait for draw first
            }
            // Draw: we don't have a specific parsed message for draws yet.
            // The server sends individual T messages. Handle from raw JSON.
            ServerMessage::Z => {} // keepalive, ignore

            ServerMessage::Agari { .. } | ServerMessage::Ryuukyoku { .. } => {
                log::info!("Round ended!");
                out.push(protocol::build_nextready());
            }

            _ => {
                // Try to detect game messages from raw JSON
                if raw.contains("\"tag\":\"T\"") {
                    let tile = parse_json_field(raw, "p");
                    let t_values = extract_t_values(raw);
                    log::info!("Draw: tile={:?} t={:?}", tile, t_values);

                    // Check for tsumo trigger (t=16 or t=48)
                    if t_values.iter().any(|v| v == "16" || v == "48") {
                        log::info!("TSUMO available!");
                        out.push(protocol::build_n(7)); // tsumo
                    } else if t_values.iter().any(|v| v == "64") {
                        log::info!("Kyushu kyuhai available");
                        out.push(protocol::build_n(9));
                    } else {
                        // Normal draw: choose discard
                        let discard = self.ai.choose_discard(&self.state);
                        log::info!("DISCARD {}", crate::tile::tile136_display(discard));
                        out.push(protocol::build_discard(discard));
                    }
                } else if raw.contains("\"tag\":\"D\"") || raw.contains("\"tag\":\"d\"") {
                    // Opponent's discard — check ron/meld
                    let t_values = extract_t_values(raw);
                    if t_values.iter().any(|v| ["8","9","10","11","12","13","15"].contains(&v.as_str())) {
                        log::info!("RON available!");
                        out.push(protocol::build_n(6));
                    } else {
                        out.push(protocol::build_n(0)); // pass
                    }
                } else if raw.contains("\"tag\":\"REACH\"") && raw.contains("\"step\":\"2\"") {
                    // Reach accepted — no response needed
                }
            }
        }

        Ok(out)
    }
}

/// Extract a single JSON field value
fn parse_json_field(json: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{}\":\"", field);
    let start = json.find(&pattern)? + pattern.len();
    let end = json[start..].find('"')?;
    Some(json[start..start + end].to_string())
}

/// Extract t= values from a JSON message (for win/meld triggers)
fn extract_t_values(json: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut search = json;
    while let Some(pos) = search.find("\"t\":\"") {
        let start = pos + 5;
        if let Some(end) = search[start..].find('"') {
            values.push(search[start..start + end].to_string());
            search = &search[start + end..];
        } else {
            break;
        }
    }
    // Also check for numeric t values: "t":16
    let mut search = json;
    while let Some(pos) = search.find("\"t\":") {
        let start = pos + 4;
        if search.as_bytes().get(start) == Some(&b'"') { search = &search[start..]; continue; }
        let end = search[start..].find(|c: char| !c.is_ascii_digit()).unwrap_or(search.len() - start);
        values.push(search[start..start + end].to_string());
        search = &search[start + end..];
    }
    values
}
