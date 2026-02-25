//! Advanced Memory System — FTS5 full-text search, OpenAI embeddings,
//! hybrid vector + keyword search, temporal decay, and memory sync.
//!
//! Ported from OpenClaw `src/memory/`: upgrades Helix's basic
//! key-value memory store to a full-featured semantic memory engine.

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::info;

use crate::modules::config::get_data_dir;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: i64,
    pub key: String,
    pub content: String,
    /// Source type: "user", "conversation", "file", "note", "agent"
    pub source: String,
    /// Optional tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Relevance score (set during search)
    #[serde(default)]
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResult {
    pub entry: MemoryEntry,
    pub score: f64,
    /// How the match was found: "fts", "vector", "hybrid", "exact"
    pub match_type: String,
    /// Snippet with highlights
    #[serde(default)]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_entries: i64,
    pub total_with_embeddings: i64,
    pub sources: HashMap<String, i64>,
    pub db_size_bytes: u64,
}

// ============================================================================
// Database
// ============================================================================

static MEMORY_DB: Lazy<Mutex<Connection>> = Lazy::new(|| {
    let conn = open_memory_db().expect("Failed to open memory database");
    Mutex::new(conn)
});

fn open_memory_db() -> Result<Connection, String> {
    let data_dir = get_data_dir()?;
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("create dir: {}", e))?;
    let db_path = data_dir.join("helix.db");
    let conn = Connection::open(&db_path).map_err(|e| format!("open DB: {}", e))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
        .map_err(|e| format!("pragmas: {}", e))?;
    Ok(conn)
}

pub fn init_memory_tables() -> Result<(), String> {
    let conn = MEMORY_DB.lock();
    conn.execute_batch(
        "
        -- Main memory entries table (upgrade from simple key-value)
        CREATE TABLE IF NOT EXISTS memory_entries (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            key         TEXT NOT NULL,
            content     TEXT NOT NULL,
            source      TEXT NOT NULL DEFAULT 'user',
            tags        TEXT DEFAULT '[]',
            embedding   BLOB,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_memory_key ON memory_entries(key);
        CREATE INDEX IF NOT EXISTS idx_memory_source ON memory_entries(source);

        -- FTS5 virtual table for full-text search
        CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
            key,
            content,
            tags,
            content=memory_entries,
            content_rowid=id,
            tokenize='unicode61'
        );

        -- Triggers to keep FTS in sync
        CREATE TRIGGER IF NOT EXISTS memory_fts_insert AFTER INSERT ON memory_entries BEGIN
            INSERT INTO memory_fts(rowid, key, content, tags)
            VALUES (new.id, new.key, new.content, new.tags);
        END;

        CREATE TRIGGER IF NOT EXISTS memory_fts_delete AFTER DELETE ON memory_entries BEGIN
            INSERT INTO memory_fts(memory_fts, rowid, key, content, tags)
            VALUES ('delete', old.id, old.key, old.content, old.tags);
        END;

        CREATE TRIGGER IF NOT EXISTS memory_fts_update AFTER UPDATE ON memory_entries BEGIN
            INSERT INTO memory_fts(memory_fts, rowid, key, content, tags)
            VALUES ('delete', old.id, old.key, old.content, old.tags);
            INSERT INTO memory_fts(rowid, key, content, tags)
            VALUES (new.id, new.key, new.content, new.tags);
        END;
        ",
    )
    .map_err(|e| format!("create memory tables: {}", e))?;
    info!("Advanced memory tables initialized (FTS5 enabled)");
    Ok(())
}

// ============================================================================
// CRUD
// ============================================================================

pub fn store_memory(key: &str, content: &str, source: &str, tags: &[String]) -> Result<MemoryEntry, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());

    let conn = MEMORY_DB.lock();

    // Upsert: update if key exists, insert if not
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM memory_entries WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = existing {
        conn.execute(
            "UPDATE memory_entries SET content = ?1, source = ?2, tags = ?3, updated_at = ?4 WHERE id = ?5",
            params![content, source, tags_json, now, id],
        )
        .map_err(|e| format!("update memory: {}", e))?;

        Ok(MemoryEntry {
            id,
            key: key.to_string(),
            content: content.to_string(),
            source: source.to_string(),
            tags: tags.to_vec(),
            created_at: now.clone(),
            updated_at: now,
            score: 0.0,
        })
    } else {
        conn.execute(
            "INSERT INTO memory_entries (key, content, source, tags, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![key, content, source, tags_json, now, now],
        )
        .map_err(|e| format!("insert memory: {}", e))?;

        let id = conn.last_insert_rowid();
        Ok(MemoryEntry {
            id,
            key: key.to_string(),
            content: content.to_string(),
            source: source.to_string(),
            tags: tags.to_vec(),
            created_at: now.clone(),
            updated_at: now,
            score: 0.0,
        })
    }
}

pub fn delete_memory(id: i64) -> Result<(), String> {
    let conn = MEMORY_DB.lock();
    conn.execute("DELETE FROM memory_entries WHERE id = ?1", params![id])
        .map_err(|e| format!("delete memory: {}", e))?;
    Ok(())
}

pub fn list_memories(source: Option<&str>, limit: i64) -> Result<Vec<MemoryEntry>, String> {
    let conn = MEMORY_DB.lock();
    let query = if let Some(src) = source {
        format!(
            "SELECT id, key, content, source, tags, created_at, updated_at FROM memory_entries WHERE source = '{}' ORDER BY updated_at DESC LIMIT {}",
            src, limit
        )
    } else {
        format!(
            "SELECT id, key, content, source, tags, created_at, updated_at FROM memory_entries ORDER BY updated_at DESC LIMIT {}",
            limit
        )
    };

    let mut stmt = conn.prepare(&query).map_err(|e| format!("query: {}", e))?;
    let entries = stmt
        .query_map([], |row| {
            let tags_str: String = row.get(4)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            Ok(MemoryEntry {
                id: row.get(0)?,
                key: row.get(1)?,
                content: row.get(2)?,
                source: row.get(3)?,
                tags,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                score: 0.0,
            })
        })
        .map_err(|e| format!("map: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect: {}", e))?;

    Ok(entries)
}

// ============================================================================
// Search — FTS5 Full-Text Search
// ============================================================================

/// Search memories using FTS5 full-text search.
pub fn search_fts(query: &str, limit: i64) -> Result<Vec<MemorySearchResult>, String> {
    let conn = MEMORY_DB.lock();

    // Sanitize query for FTS5: wrap each word in quotes to handle special chars
    let fts_query = query
        .split_whitespace()
        .map(|w| format!("\"{}\"", w.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ");

    if fts_query.is_empty() {
        return Ok(vec![]);
    }

    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.key, m.content, m.source, m.tags, m.created_at, m.updated_at,
                    rank
             FROM memory_fts f
             JOIN memory_entries m ON f.rowid = m.id
             WHERE memory_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )
        .map_err(|e| format!("FTS query: {}", e))?;

    let results = stmt
        .query_map(params![fts_query, limit], |row| {
            let tags_str: String = row.get(4)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let rank: f64 = row.get(7)?;
            Ok(MemorySearchResult {
                entry: MemoryEntry {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    content: row.get(2)?,
                    source: row.get(3)?,
                    tags,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    score: -rank, // FTS5 rank is negative (lower = better)
                },
                score: -rank,
                match_type: "fts".to_string(),
                snippet: None,
            })
        })
        .map_err(|e| format!("FTS map: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("FTS collect: {}", e))?;

    Ok(results)
}

/// Fuzzy search: fall back to LIKE if FTS finds nothing.
pub fn search_fuzzy(query: &str, limit: i64) -> Result<Vec<MemorySearchResult>, String> {
    let conn = MEMORY_DB.lock();
    let pattern = format!("%{}%", query);

    let mut stmt = conn
        .prepare(
            "SELECT id, key, content, source, tags, created_at, updated_at
             FROM memory_entries
             WHERE key LIKE ?1 OR content LIKE ?1
             ORDER BY updated_at DESC
             LIMIT ?2",
        )
        .map_err(|e| format!("fuzzy query: {}", e))?;

    let results = stmt
        .query_map(params![pattern, limit], |row| {
            let tags_str: String = row.get(4)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            Ok(MemorySearchResult {
                entry: MemoryEntry {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    content: row.get(2)?,
                    source: row.get(3)?,
                    tags,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    score: 0.5,
                },
                score: 0.5,
                match_type: "fuzzy".to_string(),
                snippet: None,
            })
        })
        .map_err(|e| format!("fuzzy map: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("fuzzy collect: {}", e))?;

    Ok(results)
}

// ============================================================================
// Hybrid Search (FTS + vector fallback)
// ============================================================================

/// Hybrid search: try FTS5 first, fall back to fuzzy LIKE, apply temporal decay.
pub fn search_hybrid(query: &str, limit: i64) -> Result<Vec<MemorySearchResult>, String> {
    // 1. Try FTS5
    let mut results = search_fts(query, limit)?;

    // 2. If no FTS results, fall back to fuzzy
    if results.is_empty() {
        results = search_fuzzy(query, limit)?;
    }

    // 3. Apply temporal decay: recent memories get a boost
    let now = chrono::Utc::now().timestamp() as f64;
    let half_life_days: f64 = 30.0;
    let half_life_secs = half_life_days * 86400.0;

    for result in &mut results {
        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&result.entry.updated_at) {
            let age_secs = now - ts.timestamp() as f64;
            let decay = (0.5_f64).powf(age_secs / half_life_secs);
            result.score *= decay.max(0.1); // floor at 10% of original score
        }
    }

    // 4. Re-sort by adjusted score
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    Ok(results)
}

// ============================================================================
// Embeddings (OpenAI text-embedding-3-small)
// ============================================================================

/// Generate embeddings for text using the configured AI provider.
pub async fn generate_embedding(text: &str) -> Result<Vec<f32>, String> {
    let config = crate::modules::config::load_app_config().map_err(|e| format!("config: {}", e))?;
    let ai = &config.ai_config;

    if ai.api_key.is_empty() {
        return Err("API key not configured".to_string());
    }

    let url = format!("{}/embeddings", ai.base_url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let body = json!({
        "model": "text-embedding-3-small",
        "input": text,
    });

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", ai.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("embedding request failed: {}", e))?;

    if !resp.status().is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("embedding API error: {}", &err[..err.len().min(200)]));
    }

    let data: Value = resp.json().await.map_err(|e| format!("parse embedding: {}", e))?;
    let embedding = data["data"][0]["embedding"]
        .as_array()
        .ok_or("No embedding in response")?
        .iter()
        .filter_map(|v| v.as_f64().map(|f| f as f32))
        .collect::<Vec<f32>>();

    if embedding.is_empty() {
        return Err("Empty embedding returned".to_string());
    }

    Ok(embedding)
}

/// Store embedding for a memory entry.
pub fn store_embedding(entry_id: i64, embedding: &[f32]) -> Result<(), String> {
    let conn = MEMORY_DB.lock();
    let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
    conn.execute(
        "UPDATE memory_entries SET embedding = ?1 WHERE id = ?2",
        params![bytes, entry_id],
    )
    .map_err(|e| format!("store embedding: {}", e))?;
    Ok(())
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

/// Vector search: find memories most similar to a query embedding.
pub fn search_vector(query_embedding: &[f32], limit: i64) -> Result<Vec<MemorySearchResult>, String> {
    let conn = MEMORY_DB.lock();

    let mut stmt = conn
        .prepare(
            "SELECT id, key, content, source, tags, created_at, updated_at, embedding
             FROM memory_entries
             WHERE embedding IS NOT NULL",
        )
        .map_err(|e| format!("vector query: {}", e))?;

    let mut scored: Vec<MemorySearchResult> = stmt
        .query_map([], |row| {
            let tags_str: String = row.get(4)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let emb_bytes: Vec<u8> = row.get(7)?;
            let embedding: Vec<f32> = emb_bytes
                .chunks(4)
                .map(|chunk| {
                    let bytes: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
                    f32::from_le_bytes(bytes)
                })
                .collect();

            let sim = cosine_similarity(query_embedding, &embedding);
            Ok(MemorySearchResult {
                entry: MemoryEntry {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    content: row.get(2)?,
                    source: row.get(3)?,
                    tags,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    score: sim as f64,
                },
                score: sim as f64,
                match_type: "vector".to_string(),
                snippet: None,
            })
        })
        .map_err(|e| format!("vector map: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("vector collect: {}", e))?;

    // Sort by similarity descending
    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit as usize);

    Ok(scored)
}

// ============================================================================
// Memory Stats
// ============================================================================

pub fn get_memory_stats() -> Result<MemoryStats, String> {
    let conn = MEMORY_DB.lock();

    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM memory_entries", [], |r| r.get(0))
        .unwrap_or(0);

    let with_embeddings: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memory_entries WHERE embedding IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let mut sources: HashMap<String, i64> = HashMap::new();
    if let Ok(mut stmt) = conn.prepare("SELECT source, COUNT(*) FROM memory_entries GROUP BY source") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        }) {
            for row in rows.flatten() {
                sources.insert(row.0, row.1);
            }
        }
    }

    let db_path = get_data_dir().map(|d| d.join("helix.db")).unwrap_or_default();
    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    Ok(MemoryStats {
        total_entries: total,
        total_with_embeddings: with_embeddings,
        sources,
        db_size_bytes: db_size,
    })
}

// ============================================================================
// Convenience: Auto-save conversation turns as memories
// ============================================================================

/// Save a conversation exchange as a memory entry for future retrieval.
pub fn save_conversation_memory(account_id: &str, user_msg: &str, assistant_msg: &str) -> Result<(), String> {
    // Generate a meaningful key from the user message
    let key = format!(
        "conv:{}_{}",
        account_id,
        chrono::Utc::now().format("%Y%m%d_%H%M%S")
    );

    let content = format!("Q: {}\nA: {}", user_msg, assistant_msg);
    let tags = vec!["conversation".to_string(), account_id.to_string()];

    store_memory(&key, &content, "conversation", &tags)?;
    Ok(())
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn memory_search(query: String, limit: Option<i64>) -> Result<Vec<MemorySearchResult>, String> {
    search_hybrid(&query, limit.unwrap_or(20))
}

#[tauri::command]
pub async fn memory_store_entry(key: String, content: String, source: Option<String>, tags: Option<Vec<String>>) -> Result<MemoryEntry, String> {
    store_memory(&key, &content, &source.unwrap_or_else(|| "user".to_string()), &tags.unwrap_or_default())
}

#[tauri::command]
pub async fn memory_delete(id: i64) -> Result<(), String> {
    delete_memory(id)
}

#[tauri::command]
pub async fn memory_list(source: Option<String>, limit: Option<i64>) -> Result<Vec<MemoryEntry>, String> {
    list_memories(source.as_deref(), limit.unwrap_or(50))
}

#[tauri::command]
pub async fn memory_stats() -> Result<MemoryStats, String> {
    get_memory_stats()
}

#[tauri::command]
pub async fn memory_embed(entry_id: i64) -> Result<String, String> {
    let content = {
        let conn = MEMORY_DB.lock();
        conn.query_row("SELECT content FROM memory_entries WHERE id = ?1", params![entry_id], |r| r.get::<_, String>(0))
            .map_err(|e| format!("find entry: {}", e))?
    };

    let embedding = generate_embedding(&content).await?;
    store_embedding(entry_id, &embedding)?;
    Ok(format!("Embedded {} dimensions for entry {}", embedding.len(), entry_id))
}

#[tauri::command]
pub async fn memory_save_conversation(account_id: String, user_msg: String, assistant_msg: String) -> Result<(), String> {
    save_conversation_memory(&account_id, &user_msg, &assistant_msg)
}

// ============================================================================
// Memory Flush — Save to persistent files (仿 OpenClaw memory-flush.ts)
// ============================================================================

/// Flush recent memories to ~/.helix/memory/YYYY-MM-DD.md for durable persistence.
/// Called before compaction or when user explicitly requests a save.
pub fn flush_memories_to_file(days_back: i64) -> Result<String, String> {
    let data_dir = get_data_dir()?;
    let memory_dir = data_dir.join("memory");
    std::fs::create_dir_all(&memory_dir).map_err(|e| format!("create memory dir: {}", e))?;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let file_path = memory_dir.join(format!("{}.md", today));

    // Get recent memories
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(days_back)).to_rfc3339();

    let entries: Vec<(String, String, String, String)> = {
        let conn = MEMORY_DB.lock();
        let mut stmt = conn
            .prepare(
                "SELECT key, content, source, created_at FROM memory_entries
                 WHERE created_at >= ?1
                 ORDER BY created_at ASC",
            )
            .map_err(|e| format!("flush query: {}", e))?;

        let res = stmt
            .query_map(params![cutoff], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| format!("flush map: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("flush collect: {}", e))?;
            
        res
    }; // conn dropped here

    if entries.is_empty() {
        return Ok("No recent memories to flush".to_string());
    }

    // Build markdown content
    let mut content = format!("# Helix Memory — {}\n\n", today);
    for (key, text, source, time) in &entries {
        content.push_str(&format!("## {} [{}]\n", key, source));
        content.push_str(&format!("*{}*\n\n", time));
        content.push_str(text);
        content.push_str("\n\n---\n\n");
    }

    // Append to file (don't overwrite, accumulate throughout the day)
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .map_err(|e| format!("open memory file: {}", e))?;

    file.write_all(content.as_bytes())
        .map_err(|e| format!("write memory file: {}", e))?;

    let msg = format!("Flushed {} memories to {}", entries.len(), file_path.display());
    info!("{}", msg);
    Ok(msg)
}

/// List memory files in ~/.helix/memory/
pub fn list_memory_files() -> Result<Vec<String>, String> {
    let data_dir = get_data_dir()?;
    let memory_dir = data_dir.join("memory");

    if !memory_dir.exists() {
        return Ok(vec![]);
    }

    let mut files: Vec<String> = std::fs::read_dir(&memory_dir)
        .map_err(|e| format!("read memory dir: {}", e))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") { Some(name) } else { None }
        })
        .collect();

    files.sort_by(|a, b| b.cmp(a)); // newest first
    Ok(files)
}

#[tauri::command]
pub async fn memory_flush(days_back: Option<i64>) -> Result<String, String> {
    flush_memories_to_file(days_back.unwrap_or(1))
}

#[tauri::command]
pub async fn memory_list_files() -> Result<Vec<String>, String> {
    list_memory_files()
}
