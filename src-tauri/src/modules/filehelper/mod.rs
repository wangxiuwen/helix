//! WeChat File Transfer Assistant (文件传输助手) — SDK module.
//!
//! Directory structure:
//! - `mod.rs`      — Types, session state, persistence, helpers
//! - `protocol.rs` — Login, messaging, sync, file upload (core WeChat protocol)
//! - `bot_api.rs`  — Telegram Bot API compatible HTTP endpoints
//! - `commands.rs` — Tauri commands for frontend

pub mod protocol;
pub mod bot_api;
pub mod commands;
pub mod bot_commands;

// Re-export public items for external use
pub use protocol::{send_text_message, send_file_to_wechat};
pub use commands::*;
pub use bot_api::bot_api_routes;

use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::redirect;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use once_cell::sync::Lazy;
use tracing::{info, warn};


// ============================================================================
// Constants
// ============================================================================

pub(crate) const WX_LOGIN_HOST: &str = "https://login.wx.qq.com";
pub(crate) const WX_QR_HOST: &str = "https://login.weixin.qq.com";
pub(crate) const WX_FILEHELPER_HOST: &str = "https://filehelper.weixin.qq.com";
pub(crate) const APP_ID: &str = "wx_webfilehelper";

/// Plaintext prefix added to all bot-sent messages.
/// Used to distinguish bot replies from user messages in file helper (self-chat).
pub(crate) const BOT_PREFIX: &str = "[Helix] ";

// ============================================================================
// Data structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileHelperSession {
    pub uin: String,
    pub sid: String,
    pub skey: String,
    pub pass_ticket: String,
    pub webwx_data_ticket: String,
    pub username: String,
    pub username_hash: String,
    pub sync_key: Option<Value>,
    pub logged_in: bool,
    pub raw_cookies: Vec<String>,
    /// Server-assigned API host from the login redirect (e.g. "szfilehelper.weixin.qq.com")
    pub api_host: String,
}

impl FileHelperSession {
    /// Returns the API base URL (e.g., "https://szfilehelper.weixin.qq.com").
    /// Falls back to WX_FILEHELPER_HOST if not set from login redirect.
    pub fn api_host_url(&self) -> &str {
        if self.api_host.is_empty() { WX_FILEHELPER_HOST } else { &self.api_host }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub content: String,
    pub from_me: bool,
    pub is_bot: bool,
    pub timestamp: u64,
    pub msg_type: i32,
}

// ============================================================================
// Multi-session state
// ============================================================================

pub struct SessionState {
    pub id: String,
    pub session: FileHelperSession,
    pub login_uuid: String,
    pub client: reqwest::Client,
    pub no_redirect_client: reqwest::Client,
    pub is_polling: Arc<AtomicBool>,
    pub last_msg_count: i64,
}

pub static SESSIONS: Lazy<Mutex<HashMap<String, SessionState>>> = Lazy::new(|| {
    Mutex::new(load_sessions_from_disk())
});

// ============================================================================
// Session persistence
// ============================================================================

fn get_sessions_path() -> std::path::PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push(".helix");
    let _ = std::fs::create_dir_all(&path);
    path.push("wechat_sessions.json");
    path
}

fn load_sessions_from_disk() -> HashMap<String, SessionState> {
    let mut result = HashMap::new();
    if let Ok(json) = std::fs::read_to_string(get_sessions_path()) {
        if let Ok(saved) = serde_json::from_str::<HashMap<String, FileHelperSession>>(&json) {
            for (id, mut session) in saved {
                // Restore webwx_data_ticket from cookies if not already set
                if session.webwx_data_ticket.is_empty() {
                    if let Some(ticket) = session.raw_cookies.iter().find_map(|c| {
                        c.split(';').next().and_then(|part| {
                            let part = part.trim();
                            if part.starts_with("webwx_data_ticket=") {
                                Some(part["webwx_data_ticket=".len()..].to_string())
                            } else {
                                None
                            }
                        })
                    }) {
                        session.webwx_data_ticket = ticket;
                    }
                }

                let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
                if let Ok(url1) = "https://wx.qq.com".parse::<reqwest::Url>() {
                    for c in &session.raw_cookies { jar.add_cookie_str(c, &url1); }
                }
                if let Ok(url2) = "https://szfilehelper.weixin.qq.com".parse::<reqwest::Url>() {
                    for c in &session.raw_cookies { jar.add_cookie_str(c, &url2); }
                }
                if let Ok(url3) = "https://login.wx.qq.com".parse::<reqwest::Url>() {
                    for c in &session.raw_cookies { jar.add_cookie_str(c, &url3); }
                }
                if let Ok(url4) = "https://file.wx.qq.com".parse::<reqwest::Url>() {
                    for c in &session.raw_cookies { jar.add_cookie_str(c, &url4); }
                }
                
                let client = reqwest::Client::builder()
                    .cookie_provider(jar)
                    .danger_accept_invalid_certs(true)
                    .http1_only()
                    .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                    .default_headers(default_headers())
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());
                    
                result.insert(id.clone(), SessionState {
                    id: id.clone(),
                    session,
                    login_uuid: String::new(),
                    client,
                    no_redirect_client: build_no_redirect_client(),
                    is_polling: Arc::new(AtomicBool::new(false)),
                    last_msg_count: 0,
                });
            }
        }
    }
    info!("Loaded {} restored wechat sessions from disk.", result.len());
    result
}

pub(crate) fn save_sessions_to_disk() {
    let sessions = SESSIONS.lock().unwrap();
    let mut to_save = HashMap::new();
    for (id, s) in sessions.iter() {
        if s.session.logged_in {
            to_save.insert(id.clone(), s.session.clone());
        }
    }
    if let Ok(json) = serde_json::to_string(&to_save) {
        let _ = std::fs::write(get_sessions_path(), json);
    }
}

// ============================================================================
// Background polling
// ============================================================================

/// Start a background tokio task that polls WeChat messages with
/// dynamic polling interval (speeds up when active, slows down when idle).
pub fn start_background_polling() {
    // Force SESSIONS lazy init so sessions are loaded from disk
    {
        let sessions = SESSIONS.lock().unwrap();
        info!("Background polling: found {} sessions", sessions.len());
    }

    tauri::async_runtime::spawn(async {
        // Dynamic polling interval (like Python background.py)
        let min_interval = 1.0_f64;   // seconds, when active
        let max_interval = 5.0_f64;   // seconds, when idle
        let mut poll_interval = 3.0_f64;

        loop {
            tokio::time::sleep(std::time::Duration::from_secs_f64(poll_interval)).await;

            // Collect logged-in session IDs
            let session_ids: Vec<String> = {
                let sessions = SESSIONS.lock().unwrap();
                sessions.iter()
                    .filter(|(_, s)| s.session.logged_in)
                    .map(|(id, _)| id.clone())
                    .collect()
            };

            let mut had_messages = false;
            for sid in session_ids {
                match commands::poll_messages_inner(&sid).await {
                    Ok(val) => {
                        if val.get("has_new").and_then(|v| v.as_bool()).unwrap_or(false) {
                            had_messages = true;
                        }
                    }
                    Err(e) => {
                        // Only log non-trivial errors
                        if !e.contains("Session not found") {
                            warn!("[bg-poll] Error polling {}: {}", &sid[..8.min(sid.len())], e);
                        }
                    }
                }
            }

            // Adapt: fast when active, slow when idle
            if had_messages {
                poll_interval = min_interval;
            } else {
                poll_interval = (poll_interval * 1.3).min(max_interval);
            }
        }
    });
}

// ============================================================================
// Helpers
// ============================================================================

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn generate_device_id() -> String {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}", seed % 1_000_000_000_000_000u128)
}

pub(crate) fn generate_msg_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = ts.as_secs();
    let nanos = ts.subsec_nanos();
    format!("{}{}{}", secs, nanos / 1000, nanos % 10)
}

pub(crate) fn default_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("accept", HeaderValue::from_static("*/*"));
    h.insert("accept-language", HeaderValue::from_static("zh,zh-CN;q=0.9,en;q=0.8"));
    h.insert("cache-control", HeaderValue::from_static("no-cache"));
    // NOTE: mmweb_appid is NOT a default header — it is only sent on specific
    // post-login API calls (webwxinit, webwxsendmsg, etc.), NOT on jslogin/poll.
    h.insert("referer", HeaderValue::from_static("https://filehelper.weixin.qq.com/"));
    h
}

pub fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .danger_accept_invalid_certs(true)
        .http1_only()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .default_headers(default_headers())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub fn build_no_redirect_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .danger_accept_invalid_certs(true)
        .http1_only()
        .redirect(redirect::Policy::none())
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .default_headers(default_headers())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub(crate) fn regex_match(pattern: &str, text: &str) -> Result<String, String> {
    let re = regex::Regex::new(pattern).map_err(|e| format!("Regex error: {}", e))?;
    re.captures(text)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| format!("Pattern not found: {} in text: {}", pattern, &text[..text.len().min(200)]))
}

pub(crate) fn get_session_snapshot(session_id: &str) -> Result<(FileHelperSession, reqwest::Client), String> {
    let sessions = SESSIONS.lock().unwrap();
    let s = sessions.get(session_id)
        .ok_or_else(|| format!("Session '{}' not found", session_id))?;
    Ok((s.session.clone(), s.client.clone()))
}

pub(crate) fn build_base_request(session: &FileHelperSession) -> Result<Value, String> {
    if !session.logged_in {
        return Err("Not logged in".to_string());
    }
    Ok(json!({
        "Uin": session.uin,
        "Sid": session.sid,
        "Skey": session.skey,
        "DeviceID": generate_device_id()
    }))
}
