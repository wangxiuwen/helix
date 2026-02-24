//! Token Usage Tracking — Unified token consumption + cost tracking.
//!
//! Every AI call (agent loop, auto-reply, manual chat) records usage here.
//! Provides per-session, per-model, daily, and total lifetime statistics.

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::info;

use super::config::get_data_dir;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEntry {
    pub id: i64,
    pub session_key: String,
    pub model: String,
    pub provider: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cost_usd: f64,
    pub source: String, // "agent", "auto_reply", "manual", "compaction"
    pub created_at: String,
}

/// Aggregate stats (lifetime or filtered).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageTotals {
    pub total_requests: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub total_cost_usd: f64,
}

/// Per-model breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model: String,
    pub provider: String,
    pub request_count: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub cost_usd: f64,
}

/// Per-day breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyUsage {
    pub date: String,
    pub request_count: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub cost_usd: f64,
}

/// Complete usage dashboard data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageDashboard {
    /// Lifetime totals
    pub totals: UsageTotals,
    /// Today's usage
    pub today: UsageTotals,
    /// Per-model breakdown
    pub by_model: Vec<ModelUsage>,
    /// Daily usage (last N days)
    pub daily: Vec<DailyUsage>,
    /// Recent entries
    pub recent: Vec<UsageEntry>,
}

// ============================================================================
// Cost Estimation
// ============================================================================

/// Estimated cost per 1M tokens (input, output) for common models.
fn model_pricing(model: &str) -> (f64, f64) {
    let m = model.to_lowercase();

    // OpenAI
    if m.starts_with("gpt-4o-mini") { return (0.15, 0.60); }
    if m.starts_with("gpt-4o") { return (2.50, 10.00); }
    if m.starts_with("gpt-4-turbo") { return (10.00, 30.00); }
    if m.starts_with("gpt-4") { return (30.00, 60.00); }
    if m.starts_with("gpt-3.5") { return (0.50, 1.50); }
    if m.starts_with("o1-mini") { return (3.00, 12.00); }
    if m.starts_with("o1") { return (15.00, 60.00); }
    if m.starts_with("o3-mini") { return (1.10, 4.40); }
    if m.starts_with("o4-mini") { return (1.10, 4.40); }

    // Anthropic
    if m.contains("claude-3-5-sonnet") || m.contains("claude-sonnet-4") { return (3.00, 15.00); }
    if m.contains("claude-3-5-haiku") || m.contains("claude-haiku-3") { return (0.80, 4.00); }
    if m.contains("claude-3-opus") || m.contains("claude-opus") { return (15.00, 75.00); }
    if m.contains("claude-3-sonnet") { return (3.00, 15.00); }
    if m.contains("claude-3-haiku") { return (0.25, 1.25); }

    // Google
    if m.starts_with("gemini-2.5-flash") || m.starts_with("gemini-2.0-flash") { return (0.10, 0.40); }
    if m.starts_with("gemini-2.5-pro") || m.starts_with("gemini-1.5-pro") { return (1.25, 5.00); }
    if m.starts_with("gemini-1.5-flash") { return (0.075, 0.30); }

    // DeepSeek
    if m.starts_with("deepseek") { return (0.14, 0.28); }

    // Qwen
    if m.starts_with("qwen") { return (0.30, 0.60); }

    // Ollama (free local)
    if m.starts_with("llama") || m.starts_with("phi") || m.starts_with("mistral") {
        return (0.0, 0.0);
    }

    // Default
    (1.00, 3.00)
}

/// Calculate estimated cost in USD.
pub fn estimate_cost(model: &str, prompt_tokens: u32, completion_tokens: u32) -> f64 {
    let (input_per_m, output_per_m) = model_pricing(model);
    let input_cost = (prompt_tokens as f64 / 1_000_000.0) * input_per_m;
    let output_cost = (completion_tokens as f64 / 1_000_000.0) * output_per_m;
    input_cost + output_cost
}

// ============================================================================
// Database
// ============================================================================

static USAGE_DB: Lazy<Mutex<rusqlite::Connection>> = Lazy::new(|| {
    let conn = open_usage_db().expect("Failed to open usage database");
    Mutex::new(conn)
});

fn open_usage_db() -> Result<rusqlite::Connection, String> {
    let data_dir = get_data_dir()?;
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("create dir: {}", e))?;
    let db_path = data_dir.join("helix.db");
    let conn = rusqlite::Connection::open(&db_path).map_err(|e| format!("open DB: {}", e))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
        .map_err(|e| format!("pragmas: {}", e))?;
    Ok(conn)
}

pub fn init_usage_tables() -> Result<(), String> {
    let conn = USAGE_DB.lock();
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS usage_log (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            session_key         TEXT NOT NULL,
            model               TEXT NOT NULL,
            provider            TEXT NOT NULL DEFAULT 'openai',
            prompt_tokens       INTEGER NOT NULL DEFAULT 0,
            completion_tokens   INTEGER NOT NULL DEFAULT 0,
            total_tokens        INTEGER NOT NULL DEFAULT 0,
            cost_usd            REAL NOT NULL DEFAULT 0.0,
            source              TEXT NOT NULL DEFAULT 'agent',
            created_at          TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_usage_session ON usage_log(session_key);
        CREATE INDEX IF NOT EXISTS idx_usage_created ON usage_log(created_at);
        CREATE INDEX IF NOT EXISTS idx_usage_model ON usage_log(model);
        ",
    )
    .map_err(|e| format!("create usage tables: {}", e))?;

    // Add source column if not exists (migration for existing DBs)
    let _ = conn.execute("ALTER TABLE usage_log ADD COLUMN source TEXT NOT NULL DEFAULT 'agent'", []);

    info!("Usage tables initialized");
    Ok(())
}

// ============================================================================
// Record Usage
// ============================================================================

/// Record a usage entry. Called by agent loop, ai_chat, and compaction.
pub fn record_usage(
    session_key: &str,
    model: &str,
    provider: &str,
    prompt_tokens: u32,
    completion_tokens: u32,
    source: &str,
) -> Result<(), String> {
    let total_tokens = prompt_tokens + completion_tokens;
    let cost = estimate_cost(model, prompt_tokens, completion_tokens);

    let conn = USAGE_DB.lock();
    conn.execute(
        "INSERT INTO usage_log (session_key, model, provider, prompt_tokens, completion_tokens, total_tokens, cost_usd, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![session_key, model, provider, prompt_tokens, completion_tokens, total_tokens, cost, source],
    )
    .map_err(|e| format!("record usage: {}", e))?;

    Ok(())
}

// ============================================================================
// Aggregation Queries
// ============================================================================

/// Get lifetime totals.
fn query_totals(where_clause: &str) -> Result<UsageTotals, String> {
    let conn = USAGE_DB.lock();
    let sql = format!(
        "SELECT COUNT(*), COALESCE(SUM(prompt_tokens),0), COALESCE(SUM(completion_tokens),0),
         COALESCE(SUM(total_tokens),0), COALESCE(SUM(cost_usd),0.0)
         FROM usage_log {}",
        where_clause
    );
    conn.query_row(&sql, [], |r| {
        Ok(UsageTotals {
            total_requests: r.get(0)?,
            total_prompt_tokens: r.get(1)?,
            total_completion_tokens: r.get(2)?,
            total_tokens: r.get(3)?,
            total_cost_usd: r.get(4)?,
        })
    })
    .map_err(|e| format!("totals: {}", e))
}

/// Get lifetime totals (all time).
pub fn get_lifetime_totals() -> Result<UsageTotals, String> {
    query_totals("")
}

/// Get today's totals.
pub fn get_today_totals() -> Result<UsageTotals, String> {
    query_totals("WHERE date(created_at) = date('now')")
}

/// Get totals for a specific session.
pub fn get_session_totals(session_key: &str) -> Result<UsageTotals, String> {
    let conn = USAGE_DB.lock();
    conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(prompt_tokens),0), COALESCE(SUM(completion_tokens),0),
         COALESCE(SUM(total_tokens),0), COALESCE(SUM(cost_usd),0.0)
         FROM usage_log WHERE session_key = ?1",
        params![session_key],
        |r| {
            Ok(UsageTotals {
                total_requests: r.get(0)?,
                total_prompt_tokens: r.get(1)?,
                total_completion_tokens: r.get(2)?,
                total_tokens: r.get(3)?,
                total_cost_usd: r.get(4)?,
            })
        },
    )
    .map_err(|e| format!("session totals: {}", e))
}

/// Get per-model breakdown.
pub fn get_model_breakdown() -> Result<Vec<ModelUsage>, String> {
    let conn = USAGE_DB.lock();
    let mut stmt = conn
        .prepare(
            "SELECT model, provider, COUNT(*), COALESCE(SUM(prompt_tokens),0),
             COALESCE(SUM(completion_tokens),0), COALESCE(SUM(total_tokens),0),
             COALESCE(SUM(cost_usd),0.0)
             FROM usage_log GROUP BY model, provider ORDER BY SUM(total_tokens) DESC",
        )
        .map_err(|e| format!("prepare: {}", e))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(ModelUsage {
                model: r.get(0)?,
                provider: r.get(1)?,
                request_count: r.get(2)?,
                prompt_tokens: r.get(3)?,
                completion_tokens: r.get(4)?,
                total_tokens: r.get(5)?,
                cost_usd: r.get(6)?,
            })
        })
        .map_err(|e| format!("query: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect: {}", e))?;

    Ok(rows)
}

/// Get daily usage for the last N days.
pub fn get_daily_usage(days: i64) -> Result<Vec<DailyUsage>, String> {
    let conn = USAGE_DB.lock();
    let mut stmt = conn
        .prepare(
            "SELECT date(created_at), COUNT(*), COALESCE(SUM(prompt_tokens),0),
             COALESCE(SUM(completion_tokens),0), COALESCE(SUM(total_tokens),0),
             COALESCE(SUM(cost_usd),0.0)
             FROM usage_log
             WHERE created_at >= datetime('now', ?1)
             GROUP BY date(created_at) ORDER BY date(created_at) DESC",
        )
        .map_err(|e| format!("prepare: {}", e))?;

    let modifier = format!("-{} days", days);
    let rows = stmt
        .query_map(params![modifier], |r| {
            Ok(DailyUsage {
                date: r.get(0)?,
                request_count: r.get(1)?,
                prompt_tokens: r.get(2)?,
                completion_tokens: r.get(3)?,
                total_tokens: r.get(4)?,
                cost_usd: r.get(5)?,
            })
        })
        .map_err(|e| format!("query: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect: {}", e))?;

    Ok(rows)
}

/// Get recent usage entries.
pub fn get_recent_usage(limit: i64) -> Result<Vec<UsageEntry>, String> {
    let conn = USAGE_DB.lock();
    let mut stmt = conn
        .prepare(
            "SELECT id, session_key, model, provider, prompt_tokens, completion_tokens,
             total_tokens, cost_usd, COALESCE(source,'agent'), created_at
             FROM usage_log ORDER BY id DESC LIMIT ?1",
        )
        .map_err(|e| format!("prepare: {}", e))?;

    let entries = stmt
        .query_map(params![limit], |r| {
            Ok(UsageEntry {
                id: r.get(0)?,
                session_key: r.get(1)?,
                model: r.get(2)?,
                provider: r.get(3)?,
                prompt_tokens: r.get(4)?,
                completion_tokens: r.get(5)?,
                total_tokens: r.get(6)?,
                cost_usd: r.get(7)?,
                source: r.get(8)?,
                created_at: r.get(9)?,
            })
        })
        .map_err(|e| format!("query: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect: {}", e))?;

    Ok(entries)
}

/// Build the complete dashboard data.
pub fn get_dashboard(recent_limit: i64, daily_days: i64) -> Result<UsageDashboard, String> {
    Ok(UsageDashboard {
        totals: get_lifetime_totals()?,
        today: get_today_totals()?,
        by_model: get_model_breakdown()?,
        daily: get_daily_usage(daily_days)?,
        recent: get_recent_usage(recent_limit)?,
    })
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Full dashboard: lifetime totals + today + model breakdown + daily + recent
#[tauri::command]
pub async fn usage_dashboard(
    recent_limit: Option<i64>,
    daily_days: Option<i64>,
) -> Result<UsageDashboard, String> {
    get_dashboard(recent_limit.unwrap_or(20), daily_days.unwrap_or(30))
}

/// Lifetime totals only
#[tauri::command]
pub async fn usage_totals() -> Result<UsageTotals, String> {
    get_lifetime_totals()
}

/// Today's totals
#[tauri::command]
pub async fn usage_today() -> Result<UsageTotals, String> {
    get_today_totals()
}

/// Per-session totals
#[tauri::command]
pub async fn usage_session(session_key: String) -> Result<UsageTotals, String> {
    get_session_totals(&session_key)
}

/// Per-model breakdown
#[tauri::command]
pub async fn usage_by_model() -> Result<Vec<ModelUsage>, String> {
    get_model_breakdown()
}

/// Daily usage history
#[tauri::command]
pub async fn usage_daily(days: Option<i64>) -> Result<Vec<DailyUsage>, String> {
    get_daily_usage(days.unwrap_or(30))
}

/// Recent usage log
#[tauri::command]
pub async fn usage_log(limit: Option<i64>) -> Result<Vec<UsageEntry>, String> {
    get_recent_usage(limit.unwrap_or(50))
}

/// Estimate cost for given tokens
#[tauri::command]
pub async fn usage_estimate_cost(model: String, prompt_tokens: u32, completion_tokens: u32) -> Result<f64, String> {
    Ok(estimate_cost(&model, prompt_tokens, completion_tokens))
}
