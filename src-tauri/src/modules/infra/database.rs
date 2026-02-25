//! SQLite database module for helix.
//!
//! Storage path follows platform conventions:
//! - macOS:   ~/Library/Application Support/helix/helix.db
//! - Linux:   ~/.local/share/helix/helix.db
//! - Windows: %APPDATA%/helix/helix.db

use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use tracing::info;

use crate::modules::config::get_data_dir;

const DB_FILE: &str = "helix.db";

/// Global database connection
static DB: Lazy<Mutex<Connection>> = Lazy::new(|| {
    let conn = open_db().expect("Failed to open database");
    Mutex::new(conn)
});

fn db_path() -> Result<PathBuf, String> {
    let dir = get_data_dir()?;
    Ok(dir.join(DB_FILE))
}

fn open_db() -> Result<Connection, String> {
    let path = db_path()?;
    info!("Opening database: {:?}", path);
    let conn = Connection::open(&path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    // Enable WAL mode for better concurrent access
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .map_err(|e| format!("Failed to set PRAGMA: {}", e))?;

    Ok(conn)
}

/// Initialize database — create tables if they don't exist.
/// Call this once at app startup.
pub fn init_db() -> Result<(), String> {
    let conn = DB.lock().map_err(|e| format!("DB lock error: {}", e))?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS accounts (
            id          TEXT PRIMARY KEY,
            nickname    TEXT NOT NULL DEFAULT '',
            remark      TEXT NOT NULL DEFAULT '',
            auto_reply  INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS messages (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id  TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
            content     TEXT NOT NULL,
            from_me     INTEGER NOT NULL DEFAULT 0,
            msg_type    INTEGER NOT NULL DEFAULT 1,
            ai_reply    INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_messages_account
            ON messages(account_id, created_at);

        CREATE TABLE IF NOT EXISTS conversation_history (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id  TEXT NOT NULL,
            role        TEXT NOT NULL DEFAULT 'user',
            content     TEXT NOT NULL,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_conv_history_account
            ON conversation_history(account_id, created_at);

        CREATE TABLE IF NOT EXISTS memory (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            key         TEXT NOT NULL,
            value       TEXT NOT NULL,
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_key ON memory(key);

        CREATE TABLE IF NOT EXISTS files (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id  TEXT NOT NULL,
            msg_id      TEXT,
            file_name   TEXT NOT NULL,
            file_path   TEXT NOT NULL,
            file_size   INTEGER DEFAULT 0,
            mime_type   TEXT,
            md5         TEXT,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_files_account ON files(account_id);
        CREATE INDEX IF NOT EXISTS idx_files_msg_id ON files(msg_id);
        "
    ).map_err(|e| format!("Failed to create tables: {}", e))?;

    info!("Database initialized at {:?}", db_path().unwrap_or_default());
    Ok(())
}


// ============================================================================
// Account operations
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Account {
    pub id: String,
    pub nickname: String,
    pub remark: String,
    pub auto_reply: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub fn create_account(id: &str, nickname: &str) -> Result<Account, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "INSERT INTO accounts (id, nickname, updated_at, auto_reply) VALUES (?1, ?2, datetime('now'), 1)
         ON CONFLICT(id) DO UPDATE SET nickname = ?2, updated_at = datetime('now')",
        params![id, nickname],
    ).map_err(|e| format!("Insert account: {}", e))?;

    get_account_inner(&conn, id)
}

pub fn update_account_nickname(id: &str, nickname: &str) -> Result<(), String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "UPDATE accounts SET nickname = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![nickname, id],
    ).map_err(|e| format!("Update nickname: {}", e))?;
    Ok(())
}

pub fn update_account_remark(id: &str, remark: &str) -> Result<(), String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "UPDATE accounts SET remark = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![remark, id],
    ).map_err(|e| format!("Update remark: {}", e))?;
    Ok(())
}

pub fn set_account_auto_reply(id: &str, enabled: bool) -> Result<(), String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "UPDATE accounts SET auto_reply = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![enabled as i32, id],
    ).map_err(|e| format!("Update auto_reply: {}", e))?;
    Ok(())
}

pub fn delete_account(id: &str) -> Result<(), String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute("DELETE FROM accounts WHERE id = ?1", params![id])
        .map_err(|e| format!("Delete account: {}", e))?;
    Ok(())
}

pub fn list_accounts() -> Result<Vec<Account>, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT id, nickname, remark, auto_reply, created_at, updated_at FROM accounts ORDER BY created_at"
    ).map_err(|e| format!("Prepare: {}", e))?;

    let rows = stmt.query_map([], |row| {
        Ok(Account {
            id: row.get(0)?,
            nickname: row.get(1)?,
            remark: row.get(2)?,
            auto_reply: row.get::<_, i32>(3)? != 0,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    }).map_err(|e| format!("Query: {}", e))?;

    let mut accounts = Vec::new();
    for row in rows {
        accounts.push(row.map_err(|e| format!("Row: {}", e))?);
    }
    Ok(accounts)
}

pub fn get_account(id: &str) -> Result<Account, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    get_account_inner(&conn, id)
}

fn get_account_inner(conn: &Connection, id: &str) -> Result<Account, String> {
    conn.query_row(
        "SELECT id, nickname, remark, auto_reply, created_at, updated_at FROM accounts WHERE id = ?1",
        params![id],
        |row| {
            Ok(Account {
                id: row.get(0)?,
                nickname: row.get(1)?,
                remark: row.get(2)?,
                auto_reply: row.get::<_, i32>(3)? != 0,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        },
    ).map_err(|e| format!("Account not found: {}", e))
}

// ============================================================================
// Message operations
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DbMessage {
    pub id: i64,
    pub account_id: String,
    pub content: String,
    pub from_me: bool,
    pub msg_type: i32,
    pub ai_reply: bool,
    pub created_at: String,
}

pub fn save_message(
    account_id: &str,
    content: &str,
    from_me: bool,
    msg_type: i32,
    ai_reply: bool,
) -> Result<i64, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "INSERT INTO messages (account_id, content, from_me, msg_type, ai_reply) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![account_id, content, from_me as i32, msg_type, ai_reply as i32],
    ).map_err(|e| format!("Insert message: {}", e))?;

    Ok(conn.last_insert_rowid())
}

/// Save a message, but only if it doesn't already exist (deduplication by content within the last 5 minutes).
pub fn save_message_dedup(
    account_id: &str,
    content: &str,
    from_me: bool,
    msg_type: i32,
    ai_reply: bool,
) -> Result<i64, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    
    // Check for recent duplicate (within 5 minutes)
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM messages 
         WHERE account_id = ?1 AND content = ?2 AND from_me = ?3 
         AND created_at > datetime('now', '-5 minutes')",
        params![account_id, content, from_me as i32],
        |row| row.get(0)
    ).unwrap_or(0);

    if count > 0 {
        return Ok(0); // Ignore duplicate
    }

    conn.execute(
        "INSERT INTO messages (account_id, content, from_me, msg_type, ai_reply) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![account_id, content, from_me as i32, msg_type, ai_reply as i32],
    ).map_err(|e| format!("Insert message: {}", e))?;

    Ok(conn.last_insert_rowid())
}

/// Get messages for an account, newest first, with limit and offset for pagination.
pub fn get_messages(account_id: &str, limit: i64, offset: i64) -> Result<Vec<DbMessage>, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT id, account_id, content, from_me, msg_type, ai_reply, created_at
         FROM messages
         WHERE account_id = ?1
         ORDER BY created_at ASC
         LIMIT ?2 OFFSET ?3"
    ).map_err(|e| format!("Prepare: {}", e))?;

    let rows = stmt.query_map(params![account_id, limit, offset], |row| {
        Ok(DbMessage {
            id: row.get(0)?,
            account_id: row.get(1)?,
            content: row.get(2)?,
            from_me: row.get::<_, i32>(3)? != 0,
            msg_type: row.get(4)?,
            ai_reply: row.get::<_, i32>(5)? != 0,
            created_at: row.get(6)?,
        })
    }).map_err(|e| format!("Query: {}", e))?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(row.map_err(|e| format!("Row: {}", e))?);
    }
    Ok(messages)
}

/// Count total messages for an account (for pagination).
pub fn count_messages(account_id: &str) -> Result<i64, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE account_id = ?1",
        params![account_id],
        |row| row.get(0),
    ).map_err(|e| format!("Count: {}", e))
}

/// Get message updates after a given offset (TG getUpdates style).
/// offset = autoincrement id, returns messages with id > offset.
pub fn get_updates(account_id: &str, offset: i64, limit: i64) -> Result<Vec<DbMessage>, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT id, account_id, content, from_me, msg_type, ai_reply, created_at
         FROM messages
         WHERE account_id = ?1 AND id > ?2
         ORDER BY id ASC
         LIMIT ?3"
    ).map_err(|e| format!("Prepare: {}", e))?;

    let rows = stmt.query_map(params![account_id, offset, limit.min(1000)], |row| {
        Ok(DbMessage {
            id: row.get(0)?,
            account_id: row.get(1)?,
            content: row.get(2)?,
            from_me: row.get::<_, i32>(3)? != 0,
            msg_type: row.get(4)?,
            ai_reply: row.get::<_, i32>(5)? != 0,
            created_at: row.get(6)?,
        })
    }).map_err(|e| format!("Query: {}", e))?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(row.map_err(|e| format!("Row: {}", e))?);
    }
    Ok(messages)
}

// ============================================================================
// File metadata operations
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DbFile {
    pub id: i64,
    pub account_id: String,
    pub msg_id: Option<String>,
    pub file_name: String,
    pub file_path: String,
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub md5: Option<String>,
    pub created_at: String,
}

/// Save file metadata to DB, returns the inserted row id.
pub fn save_file(
    account_id: &str,
    msg_id: Option<&str>,
    file_name: &str,
    file_path: &str,
    file_size: i64,
    mime_type: Option<&str>,
) -> Result<i64, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "INSERT INTO files (account_id, msg_id, file_name, file_path, file_size, mime_type)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![account_id, msg_id, file_name, file_path, file_size, mime_type],
    ).map_err(|e| format!("Insert file: {}", e))?;
    Ok(conn.last_insert_rowid())
}

/// List files for an account, newest first.
pub fn get_files(account_id: &str, limit: i64, offset: i64) -> Result<Vec<DbFile>, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT id, account_id, msg_id, file_name, file_path, file_size, mime_type, md5, created_at
         FROM files WHERE account_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
    ).map_err(|e| format!("Prepare: {}", e))?;

    let rows = stmt.query_map(params![account_id, limit, offset], |row| {
        Ok(DbFile {
            id: row.get(0)?,
            account_id: row.get(1)?,
            msg_id: row.get(2)?,
            file_name: row.get(3)?,
            file_path: row.get(4)?,
            file_size: row.get(5)?,
            mime_type: row.get(6)?,
            md5: row.get(7)?,
            created_at: row.get(8)?,
        })
    }).map_err(|e| format!("Query: {}", e))?;

    let mut files = Vec::new();
    for row in rows {
        files.push(row.map_err(|e| format!("Row: {}", e))?);
    }
    Ok(files)
}

/// Get file by its autoincrement id.
pub fn get_file_by_id(id: i64) -> Result<DbFile, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.query_row(
        "SELECT id, account_id, msg_id, file_name, file_path, file_size, mime_type, md5, created_at
         FROM files WHERE id = ?1",
        params![id],
        |row| Ok(DbFile {
            id: row.get(0)?,
            account_id: row.get(1)?,
            msg_id: row.get(2)?,
            file_name: row.get(3)?,
            file_path: row.get(4)?,
            file_size: row.get(5)?,
            mime_type: row.get(6)?,
            md5: row.get(7)?,
            created_at: row.get(8)?,
        }),
    ).map_err(|e| format!("File not found: {}", e))
}

/// Delete a file record by id.
pub fn delete_file_record(id: i64) -> Result<(), String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute("DELETE FROM files WHERE id = ?1", params![id])
        .map_err(|e| format!("Delete file: {}", e))?;
    Ok(())
}

/// Get store statistics for an account.
pub fn store_stats(account_id: &str) -> Result<serde_json::Value, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;

    let msg_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE account_id = ?1",
        params![account_id], |row| row.get(0),
    ).unwrap_or(0);

    let file_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE account_id = ?1",
        params![account_id], |row| row.get(0),
    ).unwrap_or(0);

    let total_file_size: i64 = conn.query_row(
        "SELECT COALESCE(SUM(file_size), 0) FROM files WHERE account_id = ?1",
        params![account_id], |row| row.get(0),
    ).unwrap_or(0);

    Ok(serde_json::json!({
        "account_id": account_id,
        "message_count": msg_count,
        "file_count": file_count,
        "total_file_size": total_file_size,
    }))
}

/// Delete messages older than `days` for an account.
pub fn cleanup_old_messages(account_id: &str, days: i64) -> Result<i64, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    let affected = conn.execute(
        "DELETE FROM messages WHERE account_id = ?1 AND created_at < datetime('now', ?2)",
        params![account_id, format!("-{} days", days)],
    ).map_err(|e| format!("Cleanup: {}", e))?;
    Ok(affected as i64)
}

/// Delete file records older than `days` for an account, returns list of deleted file paths.
pub fn cleanup_old_files(account_id: &str, days: i64) -> Result<Vec<String>, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;

    // First, collect paths of files to delete
    let mut stmt = conn.prepare(
        "SELECT file_path FROM files WHERE account_id = ?1 AND created_at < datetime('now', ?2)"
    ).map_err(|e| format!("Prepare: {}", e))?;

    let paths: Vec<String> = stmt.query_map(
        params![account_id, format!("-{} days", days)],
        |row| row.get(0),
    ).map_err(|e| format!("Query: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    // Then delete records
    conn.execute(
        "DELETE FROM files WHERE account_id = ?1 AND created_at < datetime('now', ?2)",
        params![account_id, format!("-{} days", days)],
    ).map_err(|e| format!("Cleanup: {}", e))?;

    Ok(paths)
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn db_list_accounts() -> Result<Vec<Account>, String> {
    list_accounts()
}

#[tauri::command]
pub async fn db_get_messages(
    account_id: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<DbMessage>, String> {
    get_messages(&account_id, limit.unwrap_or(100), offset.unwrap_or(0))
}

#[tauri::command]
pub async fn db_set_account_remark(account_id: String, remark: String) -> Result<(), String> {
    update_account_remark(&account_id, &remark)
}

#[tauri::command]
pub async fn db_set_auto_reply(account_id: String, enabled: bool) -> Result<(), String> {
    set_account_auto_reply(&account_id, enabled)
}

// ============================================================================
// Conversation History (for Agent multi-turn)
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversationEntry {
    pub id: i64,
    pub account_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

/// Save a conversation message (role: "user" | "assistant")
pub fn save_conversation_message(account_id: &str, role: &str, content: &str) -> Result<i64, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "INSERT INTO conversation_history (account_id, role, content) VALUES (?1, ?2, ?3)",
        params![account_id, role, content],
    ).map_err(|e| format!("Insert conversation: {}", e))?;
    Ok(conn.last_insert_rowid())
}

/// Get recent conversation history for an account (for context window)
pub fn get_conversation_history(account_id: &str, limit: i64) -> Result<Vec<ConversationEntry>, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    let mut stmt = conn.prepare(
        "SELECT id, account_id, role, content, created_at
         FROM conversation_history
         WHERE account_id = ?1
         ORDER BY created_at DESC
         LIMIT ?2"
    ).map_err(|e| format!("Prepare: {}", e))?;

    let rows = stmt.query_map(params![account_id, limit], |row| {
        Ok(ConversationEntry {
            id: row.get(0)?,
            account_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            created_at: row.get(4)?,
        })
    }).map_err(|e| format!("Query: {}", e))?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(|e| format!("Row: {}", e))?);
    }
    // Reverse so oldest first (we queried DESC for LIMIT)
    entries.reverse();
    Ok(entries)
}

/// Clear conversation history for an account
pub fn clear_messages(account_id: &str) -> Result<(), String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "DELETE FROM conversation_history WHERE account_id = ?1",
        params![account_id],
    ).map_err(|e| format!("Clear messages: {}", e))?;
    Ok(())
}

// ============================================================================
// Memory (long-term key-value store)
// ============================================================================

/// Store a key-value pair in memory (upsert)
pub fn memory_store(key: &str, value: &str) -> Result<(), String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    conn.execute(
        "INSERT INTO memory (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')",
        params![key, value],
    ).map_err(|e| format!("Memory store: {}", e))?;
    Ok(())
}

/// Recall memories matching a query (searches keys and values)
pub fn memory_recall(query: &str) -> Result<Vec<(String, String)>, String> {
    let conn = DB.lock().map_err(|e| format!("DB lock: {}", e))?;
    let pattern = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT key, value FROM memory WHERE key LIKE ?1 OR value LIKE ?1 ORDER BY updated_at DESC LIMIT 20"
    ).map_err(|e| format!("Prepare: {}", e))?;

    let rows = stmt.query_map(params![pattern], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }).map_err(|e| format!("Query: {}", e))?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| format!("Row: {}", e))?);
    }
    Ok(results)
}
