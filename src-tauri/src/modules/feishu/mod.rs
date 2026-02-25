//! Feishu (飞书/Lark) channel integration.
//!
//! Ported from OpenClaw @openclaw-china/feishu plugin.
//!
//! Directory structure:
//! - `mod.rs`      — Config, types, state, persistence
//! - `api.rs`      — REST API calls (token, send message, upload)
//! - `gateway.rs`  — WebSocket event subscription (receive messages)
//! - `commands.rs` — Tauri commands for frontend

pub mod api;
pub mod gateway;
pub mod commands;

pub use commands::*;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use once_cell::sync::Lazy;
use tracing::info;

// ============================================================================
// Constants
// ============================================================================

pub(crate) const FEISHU_API_BASE: &str = "https://open.feishu.cn";
/// Token refresh margin — refresh 5 minutes before expiry
pub(crate) const TOKEN_REFRESH_MARGIN_SECS: u64 = 300;

// ============================================================================
// Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuConfig {
    pub app_id: String,
    pub app_secret: String,
    #[serde(default = "default_bot_name")]
    pub bot_name: String,
    #[serde(default)]
    pub enabled: bool,
}

fn default_bot_name() -> String {
    "Helix".to_string()
}

impl Default for FeishuConfig {
    fn default() -> Self {
        Self {
            app_id: String::new(),
            app_secret: String::new(),
            bot_name: default_bot_name(),
            enabled: false,
        }
    }
}

// ============================================================================
// Token Cache
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct TokenCache {
    pub tenant_access_token: String,
    pub expires_at: u64, // Unix timestamp in seconds
}

impl TokenCache {
    pub fn is_valid(&self) -> bool {
        if self.tenant_access_token.is_empty() {
            return false;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now + TOKEN_REFRESH_MARGIN_SECS < self.expires_at
    }
}

// ============================================================================
// State
// ============================================================================

pub struct FeishuState {
    pub config: FeishuConfig,
    pub token: TokenCache,
    pub gateway_connected: bool,
    pub gateway_abort: Option<tokio::sync::watch::Sender<bool>>,
}

impl Default for FeishuState {
    fn default() -> Self {
        Self {
            config: FeishuConfig::default(),
            token: TokenCache::default(),
            gateway_connected: false,
            gateway_abort: None,
        }
    }
}

pub static FEISHU_STATE: Lazy<Arc<TokioMutex<FeishuState>>> = Lazy::new(|| {
    let state = load_config_from_disk().map(|cfg| FeishuState {
        config: cfg,
        ..Default::default()
    }).unwrap_or_default();
    Arc::new(TokioMutex::new(state))
});

// ============================================================================
// Config Persistence
// ============================================================================

fn get_config_path() -> std::path::PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push(".helix");
    let _ = std::fs::create_dir_all(&path);
    path.push("feishu.json");
    path
}

pub fn load_config_from_disk() -> Option<FeishuConfig> {
    let path = get_config_path();
    if let Ok(json) = std::fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str::<FeishuConfig>(&json) {
            info!("[Feishu] Loaded config from disk (app_id={})", &cfg.app_id);
            return Some(cfg);
        }
    }
    None
}

pub fn save_config_to_disk(cfg: &FeishuConfig) {
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(get_config_path(), json);
        info!("[Feishu] Config saved to disk");
    }
}

// ============================================================================
// Auto-start gateway if configured
// ============================================================================

/// Call this from lib.rs setup to auto-connect if feishu is enabled.
pub fn start_feishu_if_enabled() {
    tauri::async_runtime::spawn(async {
        let state = FEISHU_STATE.lock().await;
        if state.config.enabled && !state.config.app_id.is_empty() {
            let app_id = state.config.app_id.clone();
            drop(state);
            info!("[Feishu] Auto-starting gateway for app_id={}", app_id);
            if let Err(e) = gateway::start_gateway().await {
                tracing::error!("[Feishu] Auto-start failed: {}", e);
            }
        } else {
            info!("[Feishu] Not enabled or not configured, skipping auto-start");
        }
    });
}
