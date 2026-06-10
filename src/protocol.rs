//! Tenhou JSON WebSocket protocol messages.
//!
//! The new Tenhou HTML5 client uses JSON over WebSocket (wss://b-ww.mjv.jp/).
//! Messages are JSON objects with a "tag" field identifying the type.
//! No null terminators — each WebSocket text frame is one complete JSON message.

use crate::meld::Meld;
use serde::Deserialize;

// ── Incoming messages (from server) ──────────────────────────────────────

/// A parsed message from the Tenhou server.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "tag", rename_all = "UPPERCASE")]
pub enum ServerMessage {
    /// Authentication challenge response
    #[serde(rename = "HELO")]
    Helo {
        uname: Option<String>,
        ratingscale: Option<String>,
        #[serde(default)]
        auth: Option<String>,
        opt: Option<String>,
    },
    /// Lobby info (after auth)
    #[serde(rename = "LN")]
    Ln,
    /// Game found! Contains game metadata
    #[serde(rename = "GO")]
    Go {
        #[serde(rename = "type")]
        game_type: Option<String>,
        lobby: Option<String>,
        gpid: Option<String>,
    },
    /// Rejoin prompt
    #[serde(rename = "REJOIN")]
    Rejoin {
        t: Option<String>,
    },
    /// Player names and levels
    #[serde(rename = "UN")]
    Un {
        n0: Option<String>,
        n1: Option<String>,
        n2: Option<String>,
        n3: Option<String>,
        dan: Option<String>,
        rate: Option<String>,
        sx: Option<String>,
    },
    /// Game start (TAIKYOKU = "the game has begun")
    #[serde(rename = "TAIKYOKU")]
    Taikyoku {
        oya: Option<String>,
        log: Option<String>,
    },
    /// Round initialization (INIT = deal cards)
    #[serde(rename = "INIT")]
    Init {
        seed: Option<String>,
        ten: Option<String>,
        oya: Option<String>,
        hai: Option<String>,
    },
    /// Keepalive
    #[serde(rename = "Z")]
    Z,
    /// Dora indicator revealed
    #[serde(rename = "DORA")]
    Dora {
        hai: Option<String>,
    },
    /// Reach declaration
    #[serde(rename = "REACH")]
    Reach {
        who: Option<String>,
        step: Option<String>,
    },
    /// Round end (win)
    #[serde(rename = "AGARI")]
    Agari {
        who: Option<String>,
        #[serde(rename = "fromWho")]
        from_who: Option<String>,
        machi: Option<String>,
        ten: Option<String>,
        hai: Option<String>,
        yaku: Option<String>,
    },
    /// Round end (exhaustive draw)
    #[serde(rename = "RYUUKYOKU")]
    Ryuukyoku {
        ten: Option<String>,
        #[serde(rename = "hai0")]
        hai0: Option<String>,
        #[serde(rename = "hai1")]
        hai1: Option<String>,
        #[serde(rename = "hai2")]
        hai2: Option<String>,
        #[serde(rename = "hai3")]
        hai3: Option<String>,
    },
    /// Game end
    #[serde(rename = "PROF")]
    Prof,
    /// Other fields we don't specifically handle
    #[serde(other)]
    Unknown,
}

// ── Outgoing messages (to server) ────────────────────────────────────────

/// Messages we send to the Tenhou server.
pub fn build_helo(name: &str) -> String {
    serde_json::json!({"tag":"HELO","name":name,"sx":"M"}).to_string()
}

pub fn build_join(lobby: u8, game_type: u8) -> String {
    serde_json::json!({"tag":"JOIN","t":format!("{},{}", lobby, game_type)}).to_string()
}

pub fn build_gok() -> String {
    r#"{"tag":"GOK"}"#.to_string()
}

pub fn build_nextready() -> String {
    r#"{"tag":"NEXTREADY"}"#.to_string()
}

pub fn build_bye() -> String {
    r#"{"tag":"BYE"}"#.to_string()
}

pub fn build_keepalive() -> String {
    r#"{"tag":"Z"}"#.to_string()
}

/// Discard a tile. `p` is the tile in 136-format.
pub fn build_discard(tile136: u16) -> String {
    serde_json::json!({"tag":"D","p":tile136}).to_string()
}

/// Reach declaration with the discard tile.
pub fn build_reach(tile136: u16) -> String {
    serde_json::json!({"tag":"REACH","hai":tile136}).to_string()
}

/// Action response: type 0=pass, 1=pon, 2=kan, 3=chi, 4=ankan, 5=kakan, 6=ron, 7=tsumo, 9=kyushukyuhai
pub fn build_n(action_type: u8) -> String {
    serde_json::json!({"tag":"N","type":action_type}).to_string()
}

/// Meld response with hai0 and hai1
pub fn build_n_meld(action_type: u8, hai0: u16, hai1: u16) -> String {
    serde_json::json!({"tag":"N","type":action_type,"hai0":hai0,"hai1":hai1}).to_string()
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Parse a comma-separated string of ints from the server
pub fn parse_csv_i32(s: &str) -> Vec<i32> {
    s.split(',').filter_map(|n| n.parse().ok()).collect()
}

/// Parse a comma-separated string of u16s
pub fn parse_csv_u16(s: &str) -> Vec<u16> {
    s.split(',').filter_map(|n| n.parse().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_helo() {
        let msg = build_helo("NoName");
        assert!(msg.contains("HELO"));
        assert!(msg.contains("NoName"));
    }

    #[test]
    fn test_build_join() {
        let msg = build_join(0, 1);
        assert!(msg.contains("JOIN"));
    }

    #[test]
    fn test_parse_helo() {
        let json = r#"{"tag":"HELO","uname":"NoName","ratingscale":"PF3=1","opt":"3"}"#;
        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        match msg {
            ServerMessage::Helo { uname, .. } => {
                assert_eq!(uname, Some("NoName".to_string()));
            }
            _ => panic!("Expected HELO"),
        }
    }

    #[test]
    fn test_parse_go() {
        let json = r#"{"tag":"GO","type":"1","lobby":"0","gpid":"test-gpid"}"#;
        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        match msg {
            ServerMessage::Go { game_type, .. } => {
                assert_eq!(game_type, Some("1".to_string()));
            }
            _ => panic!("Expected GO"),
        }
    }
}

// ── Legacy types kept for state.rs backward compat ─────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TenhouMessage {
    Init { seed: Vec<i32>, ten: Vec<i32>, oya: u8, hai: Vec<u16>, aka: bool },
    Go { game_type: i32 },
    Un { names: [String; 4], levels: [String; 4] },
    Ln,
    GameEnd { scores: Vec<f64> },
    Helo { auth_code: Option<String>, rating: Option<String> },
    Reinit { seed: Vec<i32>, ten: Vec<i32>, oya: u8, hai: Vec<u16>, kawa: [Vec<u16>; 4], melds: [Vec<MeldInfo>; 4] },
    Draw { who: u8, tile: Option<u16> },
    Discard { who: u8, tile: u16, is_tsumogiri: bool },
    Meld { who: u8, meld: Meld, raw_m: i32 },
    Reach { who: u8, step: u8 },
    Dora { tile: u16 },
    Agari { who: u8, from_whom: u8, machi: u16, ten: Vec<i32>, hai: Vec<u16>, yaku: Vec<String> },
    Ryuukyoku { ten: Vec<i32>, revealed_hands: [Option<Vec<u16>>; 4] },
    Profile { name: String, level: String },
    Other(String),
}

#[derive(Debug, Clone)]
pub struct MeldInfo { pub meld: Meld, pub who: u8 }
