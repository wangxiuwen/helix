//! Session Management — Per-session config, model overrides, send policy,
//! and unified session tracking across all channels.
//!
//! Ported from OpenClaw `src/sessions/` and `src/channels/session.ts`.

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::modules::config::get_data_dir;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub id: i64,
    pub session_key: String,
    pub channel: String,
    pub label: Option<String>,
    pub chat_type: String, // "direct", "group", "channel"
    pub model_override: Option<String>,
    pub send_policy: String, // "allow", "deny"
    pub last_activity: String,
    pub message_count: i64,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendPolicyRule {
    pub action: String, // "allow" or "deny"
    pub match_channel: Option<String>,
    pub match_chat_type: Option<String>,
    pub match_key_prefix: Option<String>,
}

// ============================================================================
// Database
// ============================================================================

static SESSION_DB: Lazy<Mutex<rusqlite::Connection>> = Lazy::new(|| {
    let conn = open_session_db().expect("Failed to open session database");
    Mutex::new(conn)
});

fn open_session_db() -> Result<rusqlite::Connection, String> {
    let data_dir = get_data_dir()?;
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("create dir: {}", e))?;
    let db_path = data_dir.join("helix.db");
    let conn = rusqlite::Connection::open(&db_path).map_err(|e| format!("open DB: {}", e))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
        .map_err(|e| format!("pragmas: {}", e))?;
    Ok(conn)
}

pub fn init_session_tables() -> Result<(), String> {
    let conn = SESSION_DB.lock();
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            session_key     TEXT NOT NULL UNIQUE,
            channel         TEXT NOT NULL DEFAULT 'wechat_filehelper',
            label           TEXT,
            chat_type       TEXT NOT NULL DEFAULT 'direct',
            model_override  TEXT,
            send_policy     TEXT NOT NULL DEFAULT 'allow',
            last_activity   TEXT NOT NULL,
            message_count   INTEGER NOT NULL DEFAULT 0,
            metadata        TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_session_key ON sessions(session_key);
        CREATE INDEX IF NOT EXISTS idx_session_channel ON sessions(channel);
        ",
    )
    .map_err(|e| format!("create session tables: {}", e))?;
    info!("Session tables initialized");
    Ok(())
}

// ============================================================================
// CRUD
// ============================================================================

pub fn upsert_session(session_key: &str, channel: &str, label: Option<&str>) -> Result<SessionEntry, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let conn = SESSION_DB.lock();

    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM sessions WHERE session_key = ?1",
            params![session_key],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = existing {
        conn.execute(
            "UPDATE sessions SET last_activity = ?1, message_count = message_count + 1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(|e| format!("update session: {}", e))?;
    } else {
        conn.execute(
            "INSERT INTO sessions (session_key, channel, label, chat_type, send_policy, last_activity, message_count)
             VALUES (?1, ?2, ?3, 'direct', 'allow', ?4, 0)",
            params![session_key, channel, label, now],
        )
        .map_err(|e| format!("insert session: {}", e))?;
    }

    get_session(session_key)
}

pub fn get_session(session_key: &str) -> Result<SessionEntry, String> {
    let conn = SESSION_DB.lock();
    conn.query_row(
        "SELECT id, session_key, channel, label, chat_type, model_override, send_policy, last_activity, message_count, metadata
         FROM sessions WHERE session_key = ?1",
        params![session_key],
        |row| {
            Ok(SessionEntry {
                id: row.get(0)?,
                session_key: row.get(1)?,
                channel: row.get(2)?,
                label: row.get(3)?,
                chat_type: row.get(4)?,
                model_override: row.get(5)?,
                send_policy: row.get(6)?,
                last_activity: row.get(7)?,
                message_count: row.get(8)?,
                metadata: row.get(9)?,
            })
        },
    )
    .map_err(|e| format!("get session: {}", e))
}

pub fn list_sessions(channel: Option<&str>, limit: i64) -> Result<Vec<SessionEntry>, String> {
    let conn = SESSION_DB.lock();
    let query = if let Some(ch) = channel {
        format!(
            "SELECT id, session_key, channel, label, chat_type, model_override, send_policy, last_activity, message_count, metadata
             FROM sessions WHERE channel = '{}' ORDER BY last_activity DESC LIMIT {}",
            ch, limit
        )
    } else {
        format!(
            "SELECT id, session_key, channel, label, chat_type, model_override, send_policy, last_activity, message_count, metadata
             FROM sessions ORDER BY last_activity DESC LIMIT {}",
            limit
        )
    };

    let mut stmt = conn.prepare(&query).map_err(|e| format!("query: {}", e))?;
    let entries = stmt
        .query_map([], |row| {
            Ok(SessionEntry {
                id: row.get(0)?,
                session_key: row.get(1)?,
                channel: row.get(2)?,
                label: row.get(3)?,
                chat_type: row.get(4)?,
                model_override: row.get(5)?,
                send_policy: row.get(6)?,
                last_activity: row.get(7)?,
                message_count: row.get(8)?,
                metadata: row.get(9)?,
            })
        })
        .map_err(|e| format!("map: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect: {}", e))?;

    Ok(entries)
}

pub fn set_model_override(session_key: &str, model: Option<&str>) -> Result<(), String> {
    let conn = SESSION_DB.lock();
    conn.execute(
        "UPDATE sessions SET model_override = ?1 WHERE session_key = ?2",
        params![model, session_key],
    )
    .map_err(|e| format!("set model override: {}", e))?;
    Ok(())
}

pub fn set_send_policy(session_key: &str, policy: &str) -> Result<(), String> {
    if policy != "allow" && policy != "deny" {
        return Err(format!("Invalid policy: {}. Must be 'allow' or 'deny'", policy));
    }
    let conn = SESSION_DB.lock();
    conn.execute(
        "UPDATE sessions SET send_policy = ?1 WHERE session_key = ?2",
        params![policy, session_key],
    )
    .map_err(|e| format!("set send policy: {}", e))?;
    Ok(())
}

pub fn set_session_label(session_key: &str, label: &str) -> Result<(), String> {
    let conn = SESSION_DB.lock();
    conn.execute(
        "UPDATE sessions SET label = ?1 WHERE session_key = ?2",
        params![label, session_key],
    )
    .map_err(|e| format!("set label: {}", e))?;
    Ok(())
}

pub fn delete_session(session_key: &str) -> Result<(), String> {
    let conn = SESSION_DB.lock();
    conn.execute("DELETE FROM sessions WHERE session_key = ?1", params![session_key])
        .map_err(|e| format!("delete session: {}", e))?;
    Ok(())
}

/// Resolve send policy for a session: check session-level override, then defaults.
pub fn resolve_send_policy(session_key: &str) -> String {
    match get_session(session_key) {
        Ok(entry) => entry.send_policy,
        Err(_) => "allow".to_string(),
    }
}

/// Get model override for a session, or None for default.
pub fn get_model_for_session(session_key: &str) -> Option<String> {
    get_session(session_key).ok().and_then(|e| e.model_override)
}

// ============================================================================
// Conversation Compaction
// ============================================================================

/// Compact conversation history by summarizing old turns.
/// Keeps the most recent `keep_recent` messages and summarizes the rest
/// into a single context injection at the start of history.
pub async fn compact_session_history(
    account_id: &str,
    keep_recent: i64,
) -> Result<String, String> {
    use crate::modules::database;
    use crate::modules::config::load_app_config;

    // 1. Load full history
    let history = database::get_conversation_history(account_id, 200)?;
    let total = history.len() as i64;

    if total <= keep_recent {
        return Ok(format!("No compaction needed ({} messages, threshold {})", total, keep_recent));
    }

    // 2. Build summary of old messages
    let old_count = (total - keep_recent) as usize;
    let old_messages: Vec<String> = history[..old_count]
        .iter()
        .map(|m| format!("[{}]: {}", m.role, &m.content[..m.content.len().min(200)]))
        .collect();

    let summary_input = old_messages.join("\n");

    // 3. Use AI to generate a summary
    let config = load_app_config().map_err(|e| format!("config: {}", e))?;
    let ai = &config.ai_config;

    if ai.api_key.is_empty() {
        // Fallback: use simple text truncation
        let summary = format!(
            "[Compacted {} older messages. Key topics discussed: {}]",
            old_count,
            &summary_input[..summary_input.len().min(500)]
        );
        // Delete old messages and inject summary
        database::clear_messages(account_id)?;
        let _ = database::save_conversation_message(account_id, "system", &summary);
        // Re-save recent messages
        for m in &history[old_count..] {
            let _ = database::save_conversation_message(account_id, &m.role, &m.content);
        }
        return Ok(format!("Compacted {} messages (fallback mode)", old_count));
    }

    // Use the streaming engine for summarization
    let provider = crate::modules::providers::resolve_provider_config(
        &ai.model,
        Some(&ai.base_url),
        Some(&ai.api_key),
        None,
    );

    let summarize_prompt = format!(
        "Summarize the following conversation history into a brief context paragraph (2-3 sentences). \
         Focus on key facts, decisions, and ongoing tasks. Be concise.\n\n{}",
        &summary_input[..summary_input.len().min(3000)]
    );

    let body = crate::modules::providers::build_openai_request(
        &ai.model,
        &[serde_json::json!({"role": "user", "content": summarize_prompt})],
        None,
        300,
        false,
    );

    let result = crate::modules::streaming::complete_simple(&provider, &body).await?;
    let summary = if result.content.is_empty() {
        format!("[Compacted {} older messages]", old_count)
    } else {
        format!("[Context from {} earlier messages: {}]", old_count, result.content)
    };

    // 4. Replace history: delete all, inject summary + recent
    database::clear_messages(account_id)?;
    let _ = database::save_conversation_message(account_id, "system", &summary);
    for m in &history[old_count..] {
        let _ = database::save_conversation_message(account_id, &m.role, &m.content);
    }

    info!(
        "[sessions] Compacted {} old messages for '{}', kept {} recent",
        old_count, account_id, keep_recent
    );

    Ok(format!("Compacted {} messages, kept {} recent", old_count, keep_recent))
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn sessions_list(channel: Option<String>, limit: Option<i64>) -> Result<Vec<SessionEntry>, String> {
    list_sessions(channel.as_deref(), limit.unwrap_or(50))
}

#[tauri::command]
pub async fn sessions_get(session_key: String) -> Result<SessionEntry, String> {
    get_session(&session_key)
}

#[tauri::command]
pub async fn sessions_set_model(session_key: String, model: Option<String>) -> Result<(), String> {
    set_model_override(&session_key, model.as_deref())
}

#[tauri::command]
pub async fn sessions_set_policy(session_key: String, policy: String) -> Result<(), String> {
    set_send_policy(&session_key, &policy)
}

#[tauri::command]
pub async fn sessions_set_label(session_key: String, label: String) -> Result<(), String> {
    set_session_label(&session_key, &label)
}

#[tauri::command]
pub async fn sessions_delete(session_key: String) -> Result<(), String> {
    delete_session(&session_key)
}

#[tauri::command]
pub async fn sessions_compact(account_id: String, keep_recent: Option<i64>) -> Result<String, String> {
    compact_session_history(&account_id, keep_recent.unwrap_or(20)).await
}

