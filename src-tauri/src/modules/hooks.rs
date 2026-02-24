//! Hooks / Triggers — Event-driven automation.
//!
//! Simplified port from OpenClaw `src/hooks/`: register hooks that
//! fire on specific events (cron_complete, wechat_message, agent_reply).

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn, error};

use super::config::get_data_dir;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Trigger event: "cron_complete", "wechat_message", "agent_reply", "manual"
    pub trigger: String,
    /// Filter condition (JSON, e.g. {"task_name": "backup"})
    #[serde(default)]
    pub filter: Option<Value>,
    /// Action: "script" or "notify"
    pub action_type: String,
    /// Shell script or notification body template
    pub action_payload: String,
    pub enabled: bool,
    /// Optional notification channel
    #[serde(default)]
    pub notify_channel: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHookInput {
    pub name: String,
    pub description: Option<String>,
    pub trigger: String,
    pub filter: Option<Value>,
    pub action_type: String,
    pub action_payload: String,
    pub notify_channel: Option<String>,
}

// ============================================================================
// Database
// ============================================================================

static HOOKS_DB: Lazy<Mutex<Connection>> = Lazy::new(|| {
    let conn = open_hooks_db().expect("Failed to open hooks database");
    Mutex::new(conn)
});

fn open_hooks_db() -> Result<Connection, String> {
    let data_dir = get_data_dir()?;
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("create dir: {}", e))?;
    let db_path = data_dir.join("helix.db");
    let conn = Connection::open(&db_path).map_err(|e| format!("open DB: {}", e))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
        .map_err(|e| format!("pragmas: {}", e))?;
    Ok(conn)
}

pub fn init_hooks_tables() -> Result<(), String> {
    let conn = HOOKS_DB.lock();
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS hooks (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            description     TEXT NOT NULL DEFAULT '',
            trigger         TEXT NOT NULL,
            filter          TEXT,
            action_type     TEXT NOT NULL DEFAULT 'script',
            action_payload  TEXT NOT NULL DEFAULT '',
            enabled         INTEGER NOT NULL DEFAULT 1,
            notify_channel  TEXT,
            created_at      TEXT NOT NULL
        );
        ",
    )
    .map_err(|e| format!("create hooks table: {}", e))?;
    info!("Hooks tables initialized");
    Ok(())
}

// ============================================================================
// CRUD
// ============================================================================

pub fn create_hook(input: CreateHookInput) -> Result<Hook, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let filter_str = input.filter.as_ref().map(|f| f.to_string());

    let name = input.name.clone();
    let description = input.description.clone().unwrap_or_default();
    let trigger = input.trigger.clone();
    let action_type = input.action_type.clone();
    let action_payload = input.action_payload.clone();
    let notify_channel = input.notify_channel.clone();

    let conn = HOOKS_DB.lock();
    conn.execute(
        "INSERT INTO hooks (id, name, description, trigger, filter, action_type, action_payload, notify_channel, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, input.name, description, input.trigger, filter_str, input.action_type, input.action_payload, input.notify_channel, now],
    )
    .map_err(|e| format!("create hook: {}", e))?;

    info!("Created hook: {} ({})", name, id);

    Ok(Hook {
        id,
        name,
        description,
        trigger,
        filter: input.filter,
        action_type,
        action_payload,
        enabled: true,
        notify_channel,
        created_at: now,
    })
}

pub fn list_hooks() -> Result<Vec<Hook>, String> {
    let conn = HOOKS_DB.lock();
    let mut stmt = conn
        .prepare("SELECT id, name, description, trigger, filter, action_type, action_payload, enabled, notify_channel, created_at FROM hooks ORDER BY created_at DESC")
        .map_err(|e| format!("query: {}", e))?;

    let hooks = stmt
        .query_map([], |row| {
            let filter_str: Option<String> = row.get(4)?;
            let filter = filter_str.and_then(|s| serde_json::from_str(&s).ok());
            Ok(Hook {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                trigger: row.get(3)?,
                filter,
                action_type: row.get(5)?,
                action_payload: row.get(6)?,
                enabled: row.get::<_, i32>(7)? != 0,
                notify_channel: row.get(8)?,
                created_at: row.get(9)?,
            })
        })
        .map_err(|e| format!("map: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect: {}", e))?;

    Ok(hooks)
}

pub fn toggle_hook(id: &str, enabled: bool) -> Result<(), String> {
    let conn = HOOKS_DB.lock();
    conn.execute(
        "UPDATE hooks SET enabled = ?1 WHERE id = ?2",
        params![enabled as i32, id],
    )
    .map_err(|e| format!("toggle: {}", e))?;
    Ok(())
}

pub fn delete_hook(id: &str) -> Result<(), String> {
    let conn = HOOKS_DB.lock();
    conn.execute("DELETE FROM hooks WHERE id = ?1", params![id])
        .map_err(|e| format!("delete: {}", e))?;
    info!("Deleted hook: {}", id);
    Ok(())
}

// ============================================================================
// Event Dispatch
// ============================================================================

/// Dispatch an event to all matching, enabled hooks.
///
/// ```ignore
/// hooks::dispatch_event("cron_complete", json!({"task": "backup", "result": "success"})).await;
/// ```
pub async fn dispatch_event(event_type: &str, context: Value) {
    let hooks = match list_hooks() {
        Ok(h) => h,
        Err(e) => {
            error!("hooks dispatch: failed to list hooks: {}", e);
            return;
        }
    };

    for hook in hooks {
        if !hook.enabled || hook.trigger != event_type {
            continue;
        }

        // Check filter conditions if set
        if let Some(ref filter) = hook.filter {
            if let Some(filter_obj) = filter.as_object() {
                let mut matches = true;
                for (k, v) in filter_obj {
                    if context.get(k) != Some(v) {
                        matches = false;
                        break;
                    }
                }
                if !matches {
                    continue;
                }
            }
        }

        info!("Hook '{}' fired for event '{}'", hook.name, event_type);

        match hook.action_type.as_str() {
            "script" => {
                let payload = hook.action_payload.clone();
                tokio::spawn(async move {
                    let output = tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(&payload)
                        .output()
                        .await;
                    match output {
                        Ok(o) => {
                            let out = String::from_utf8_lossy(&o.stdout);
                            info!("Hook script output: {}", &out[..out.len().min(500)]);
                        }
                        Err(e) => error!("Hook script failed: {}", e),
                    }
                });
            }
            "notify" => {
                if let Some(ref channel) = hook.notify_channel {
                    let title = format!("🪝 Hook: {}", hook.name);
                    let body = hook.action_payload.clone();
                    let ch = channel.clone();
                    tokio::spawn(async move {
                        if let Err(e) = super::notifications::send_notification(&ch, &title, &body).await {
                            error!("Hook notification failed: {}", e);
                        }
                    });
                }
            }
            _ => warn!("Unknown hook action type: {}", hook.action_type),
        }
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn hooks_list() -> Result<Vec<Hook>, String> {
    list_hooks()
}

#[tauri::command]
pub async fn hooks_create(input: CreateHookInput) -> Result<Hook, String> {
    create_hook(input)
}

#[tauri::command]
pub async fn hooks_toggle(id: String, enabled: bool) -> Result<(), String> {
    toggle_hook(&id, enabled)
}

#[tauri::command]
pub async fn hooks_delete(id: String) -> Result<(), String> {
    delete_hook(&id)
}
