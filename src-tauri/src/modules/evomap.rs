//! EvoMap GEP-A2A Protocol Integration
//!
//! Implements the GEP-A2A v1.0.0 protocol for connecting to the EvoMap
//! collaborative evolution marketplace. Agents publish validated solutions
//! (Gene + Capsule bundles) and fetch promoted assets from other agents.
//!
//! Hub URL: https://evomap.ai
//! Protocol: GEP-A2A v1.0.0

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::info;

use super::config::get_data_dir;

// ============================================================================
// Types
// ============================================================================

const HUB_URL: &str = "https://evomap.ai";
const PROTOCOL: &str = "gep-a2a";
const PROTOCOL_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvoMapConfig {
    pub enabled: bool,
    pub node_id: String,
    pub claim_code: Option<String>,
    pub claim_url: Option<String>,
    pub last_sync: Option<String>,
    pub sync_interval_hours: u64,
}

impl Default for EvoMapConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            node_id: generate_node_id(),
            claim_code: None,
            claim_url: None,
            last_sync: None,
            sync_interval_hours: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvoAsset {
    pub asset_id: String,
    pub asset_type: String, // "Gene", "Capsule", "EvolutionEvent"
    pub summary: String,
    pub status: String,     // "candidate", "promoted", "quarantined"
    pub data: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvoMapStatus {
    pub enabled: bool,
    pub node_id: String,
    pub claimed: bool,
    pub claim_url: Option<String>,
    pub last_sync: Option<String>,
    pub local_assets: i64,
    pub fetched_assets: i64,
}

// ============================================================================
// Database
// ============================================================================

static EVO_DB: Lazy<Mutex<Connection>> = Lazy::new(|| {
    let conn = open_evo_db().expect("Failed to open evomap database");
    Mutex::new(conn)
});

fn open_evo_db() -> Result<Connection, String> {
    let data_dir = get_data_dir()?;
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("create dir: {}", e))?;
    let db_path = data_dir.join("evomap.db");
    let conn = Connection::open(&db_path).map_err(|e| format!("open evomap DB: {}", e))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
        .map_err(|e| format!("pragmas: {}", e))?;
    Ok(conn)
}

pub fn init_evomap_tables() -> Result<(), String> {
    let conn = EVO_DB.lock();
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS evo_config (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS evo_assets (
            asset_id   TEXT PRIMARY KEY,
            asset_type TEXT NOT NULL,
            summary    TEXT NOT NULL,
            status     TEXT NOT NULL DEFAULT 'candidate',
            data       TEXT NOT NULL,
            source     TEXT NOT NULL DEFAULT 'local',
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_evo_assets_type ON evo_assets(asset_type);
        CREATE INDEX IF NOT EXISTS idx_evo_assets_status ON evo_assets(status);
        ",
    )
    .map_err(|e| format!("create evomap tables: {}", e))?;
    info!("EvoMap tables initialized");
    Ok(())
}

// ============================================================================
// Config Helpers
// ============================================================================

fn get_config_value(key: &str) -> Option<String> {
    let conn = EVO_DB.lock();
    conn.query_row(
        "SELECT value FROM evo_config WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .ok()
}

fn set_config_value(key: &str, value: &str) -> Result<(), String> {
    let conn = EVO_DB.lock();
    conn.execute(
        "INSERT OR REPLACE INTO evo_config (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .map_err(|e| format!("set config: {}", e))?;
    Ok(())
}

fn get_or_create_node_id() -> String {
    if let Some(id) = get_config_value("node_id") {
        id
    } else {
        let id = generate_node_id();
        let _ = set_config_value("node_id", &id);
        id
    }
}

fn generate_node_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("node_{:x}{:08x}", ts, rand_u32())
}

fn generate_message_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("msg_{}_{:08x}", ts, rand_u32())
}

fn rand_u32() -> u32 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    RandomState::new().build_hasher().finish() as u32
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// ============================================================================
// Protocol Envelope
// ============================================================================

fn build_envelope(message_type: &str, sender_id: &str, payload: Value) -> Value {
    json!({
        "protocol": PROTOCOL,
        "protocol_version": PROTOCOL_VERSION,
        "message_type": message_type,
        "message_id": generate_message_id(),
        "sender_id": sender_id,
        "timestamp": now_iso(),
        "payload": payload
    })
}

// ============================================================================
// A2A Protocol Operations
// ============================================================================

/// POST /a2a/hello — Register node with EvoMap hub
pub async fn hello() -> Result<Value, String> {
    let node_id = get_or_create_node_id();

    let payload = json!({
        "capabilities": {},
        "gene_count": 0,
        "capsule_count": 0,
        "env_fingerprint": {
            "platform": std::env::consts::OS,
            "arch": std::env::consts::ARCH
        }
    });

    let envelope = build_envelope("hello", &node_id, payload);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let resp = client
        .post(format!("{}/a2a/hello", HUB_URL))
        .header("Content-Type", "application/json")
        .json(&envelope)
        .send()
        .await
        .map_err(|e| format!("hello request failed: {}", e))?;

    let data: Value = resp.json().await.map_err(|e| format!("parse hello response: {}", e))?;

    // Save claim code if returned
    if let Some(code) = data.get("claim_code").and_then(|v| v.as_str()) {
        let _ = set_config_value("claim_code", code);
    }
    if let Some(url) = data.get("claim_url").and_then(|v| v.as_str()) {
        let _ = set_config_value("claim_url", url);
    }

    info!("EvoMap hello: node_id={}", node_id);
    Ok(data)
}

/// POST /a2a/fetch — Fetch promoted assets from hub
pub async fn fetch_assets(asset_type: Option<&str>) -> Result<Vec<EvoAsset>, String> {
    let node_id = get_or_create_node_id();

    let mut payload = json!({});
    if let Some(at) = asset_type {
        payload["asset_type"] = json!(at);
    }

    let envelope = build_envelope("fetch", &node_id, payload);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let resp = client
        .post(format!("{}/a2a/fetch", HUB_URL))
        .header("Content-Type", "application/json")
        .json(&envelope)
        .send()
        .await
        .map_err(|e| format!("fetch request failed: {}", e))?;

    let data: Value = resp.json().await.map_err(|e| format!("parse fetch response: {}", e))?;

    let assets_arr = data.get("assets").and_then(|a| a.as_array());
    let mut assets = Vec::new();

    if let Some(arr) = assets_arr {
        let conn = EVO_DB.lock();
        let now = now_iso();

        for item in arr {
            let asset_id = item.get("asset_id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let asset_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("unknown");
            let summary = item.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("promoted");

            // Store locally
            let data_str = serde_json::to_string(item).unwrap_or_default();
            let _ = conn.execute(
                "INSERT OR REPLACE INTO evo_assets (asset_id, asset_type, summary, status, data, source, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'remote', ?6)",
                params![asset_id, asset_type, summary, status, data_str, now],
            );

            assets.push(EvoAsset {
                asset_id: asset_id.to_string(),
                asset_type: asset_type.to_string(),
                summary: summary.to_string(),
                status: status.to_string(),
                data: item.clone(),
                created_at: now.clone(),
            });
        }
    }

    let _ = set_config_value("last_sync", &now_iso());
    info!("EvoMap fetch: got {} assets", assets.len());
    Ok(assets)
}

/// POST /a2a/publish — Publish Gene + Capsule bundle to hub
pub async fn publish_bundle(gene: Value, capsule: Value, evolution_event: Option<Value>) -> Result<Value, String> {
    let node_id = get_or_create_node_id();

    let mut assets = vec![gene, capsule];
    if let Some(ev) = evolution_event {
        assets.push(ev);
    }

    let payload = json!({ "assets": assets });
    let envelope = build_envelope("publish", &node_id, payload);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let resp = client
        .post(format!("{}/a2a/publish", HUB_URL))
        .header("Content-Type", "application/json")
        .json(&envelope)
        .send()
        .await
        .map_err(|e| format!("publish request failed: {}", e))?;

    let data: Value = resp.json().await.map_err(|e| format!("parse publish response: {}", e))?;
    info!("EvoMap publish: {:?}", data);
    Ok(data)
}

// ============================================================================
// Local Asset Management
// ============================================================================

pub fn list_local_assets(asset_type: Option<&str>, limit: i64) -> Result<Vec<EvoAsset>, String> {
    let conn = EVO_DB.lock();

    let query = if let Some(at) = asset_type {
        format!(
            "SELECT asset_id, asset_type, summary, status, data, created_at FROM evo_assets WHERE asset_type = '{}' ORDER BY created_at DESC LIMIT {}",
            at, limit
        )
    } else {
        format!(
            "SELECT asset_id, asset_type, summary, status, data, created_at FROM evo_assets ORDER BY created_at DESC LIMIT {}",
            limit
        )
    };

    let mut stmt = conn.prepare(&query).map_err(|e| format!("query: {}", e))?;
    let assets = stmt
        .query_map([], |row| {
            let data_str: String = row.get(4)?;
            let data: Value = serde_json::from_str(&data_str).unwrap_or(json!({}));
            Ok(EvoAsset {
                asset_id: row.get(0)?,
                asset_type: row.get(1)?,
                summary: row.get(2)?,
                status: row.get(3)?,
                data,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| format!("map: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect: {}", e))?;

    Ok(assets)
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn evomap_hello() -> Result<Value, String> {
    hello().await
}

#[tauri::command]
pub async fn evomap_fetch(asset_type: Option<String>) -> Result<Vec<EvoAsset>, String> {
    fetch_assets(asset_type.as_deref()).await
}

#[tauri::command]
pub async fn evomap_publish(gene: Value, capsule: Value, evolution_event: Option<Value>) -> Result<Value, String> {
    publish_bundle(gene, capsule, evolution_event).await
}

#[tauri::command]
pub async fn evomap_list_assets(asset_type: Option<String>, limit: Option<i64>) -> Result<Vec<EvoAsset>, String> {
    list_local_assets(asset_type.as_deref(), limit.unwrap_or(50))
}

#[tauri::command]
pub async fn evomap_status() -> Result<EvoMapStatus, String> {
    let _ = init_evomap_tables();
    let node_id = get_or_create_node_id();
    let claim_url = get_config_value("claim_url");
    let last_sync = get_config_value("last_sync");
    let enabled = get_config_value("enabled").map(|v| v == "true").unwrap_or(false);

    let conn = EVO_DB.lock();
    let local_assets: i64 = conn
        .query_row("SELECT COUNT(*) FROM evo_assets WHERE source = 'local'", [], |r| r.get(0))
        .unwrap_or(0);
    let fetched_assets: i64 = conn
        .query_row("SELECT COUNT(*) FROM evo_assets WHERE source = 'remote'", [], |r| r.get(0))
        .unwrap_or(0);

    Ok(EvoMapStatus {
        enabled,
        node_id,
        claimed: claim_url.is_some(),
        claim_url,
        last_sync,
        local_assets,
        fetched_assets,
    })
}

#[tauri::command]
pub async fn evomap_toggle(enabled: bool) -> Result<(), String> {
    let _ = init_evomap_tables();
    set_config_value("enabled", if enabled { "true" } else { "false" })
}
