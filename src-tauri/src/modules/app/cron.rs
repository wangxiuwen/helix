//! Cron Job Backend — Persistent scheduled task management.
//!
//! Ported from OpenClaw `src/cron/`: persistent storage in SQLite,
//! cron expression scheduling, task execution (shell + agent), and
//! notification dispatch on completion.

use chrono::{DateTime, Timelike, Utc};
use cron::Schedule;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{info, error, warn};

use crate::modules::config::get_data_dir;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronTask {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub task_type: String, // "cron" | "manual"
    pub schedule: Option<String>, // cron expression
    pub script: Option<String>,   // shell command or AI prompt
    pub status: String,           // "active" | "paused" | "error"
    pub notify_channel: Option<String>, // "feishu" | "dingtalk" | null
    pub created_at: String,
    pub updated_at: String,
    pub last_run: Option<String>,
    pub last_result: Option<String>, // "success" | "error"
    pub next_run: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronRun {
    pub id: i64,
    pub task_id: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub result: String, // "success" | "error" | "running"
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskInput {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub task_type: String,
    pub schedule: Option<String>,
    pub script: Option<String>,
    pub notify_channel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub schedule: Option<String>,
    pub script: Option<String>,
    pub status: Option<String>,
    pub notify_channel: Option<Value>, // can be string or null
}

// ============================================================================
// Database
// ============================================================================

static CRON_DB: Lazy<Mutex<Connection>> = Lazy::new(|| {
    let conn = open_cron_db().expect("Failed to open cron database");
    Mutex::new(conn)
});

fn open_cron_db() -> Result<Connection, String> {
    let data_dir = get_data_dir()?;
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create data dir: {}", e))?;
    let db_path = data_dir.join("helix.db");
    let conn =
        Connection::open(&db_path).map_err(|e| format!("Failed to open cron DB: {}", e))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
        .map_err(|e| format!("Failed to set pragmas: {}", e))?;
    Ok(conn)
}

pub fn init_cron_tables() -> Result<(), String> {
    let conn = CRON_DB.lock();

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS cron_tasks (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            task_type   TEXT NOT NULL DEFAULT 'cron',
            schedule    TEXT,
            script      TEXT,
            status      TEXT NOT NULL DEFAULT 'active',
            notify_channel TEXT,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL,
            last_run    TEXT,
            last_result TEXT
        );

        CREATE TABLE IF NOT EXISTS cron_runs (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id     TEXT NOT NULL,
            started_at  TEXT NOT NULL,
            finished_at TEXT,
            result      TEXT NOT NULL DEFAULT 'running',
            output      TEXT NOT NULL DEFAULT '',
            FOREIGN KEY (task_id) REFERENCES cron_tasks(id) ON DELETE CASCADE
        );
        ",
    )
    .map_err(|e| format!("Failed to create cron tables: {}", e))?;

    info!("Cron tables initialized");
    Ok(())
}

// ============================================================================
// Cron Scheduling Helpers
// ============================================================================

/// Parse a cron expression and compute the next run time.
pub fn compute_next_run(cron_expr: &str) -> Option<String> {
    // Support 5/6/7 field expressions
    let expr = normalize_cron_expr(cron_expr);
    match Schedule::from_str(&expr) {
        Ok(schedule) => {
            let upcoming = schedule.upcoming(Utc).next()?;
            Some(upcoming.to_rfc3339())
        }
        Err(e) => {
            warn!("Invalid cron expression '{}': {}", cron_expr, e);
            None
        }
    }
}

/// Normalize a 5-field cron (min hour dom mon dow) to 7-field (sec min hour dom mon dow year)
/// by prepending "0" for seconds and appending "*" for year if needed.
fn normalize_cron_expr(expr: &str) -> String {
    let parts: Vec<&str> = expr.trim().split_whitespace().collect();
    match parts.len() {
        5 => format!("0 {} *", expr.trim()),
        6 => format!("0 {}", expr.trim()),
        7 => expr.trim().to_string(),
        _ => expr.trim().to_string(),
    }
}

/// Check if a cron expression is valid.
pub fn validate_cron_expr(expr: &str) -> Result<(), String> {
    let normalized = normalize_cron_expr(expr);
    Schedule::from_str(&normalized).map_err(|e| format!("Invalid cron expression: {}", e))?;
    Ok(())
}

// ============================================================================
// CRUD Operations
// ============================================================================

pub fn create_task(input: CreateTaskInput) -> Result<CronTask, String> {
    // Validate cron expression if provided
    if input.task_type == "cron" {
        if let Some(ref schedule) = input.schedule {
            if !schedule.is_empty() {
                validate_cron_expr(schedule)?;
            }
        }
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let next_run = input
        .schedule
        .as_ref()
        .and_then(|s| compute_next_run(s));

    // Clone fields before they are consumed by the SQL insert
    let name = input.name.clone();
    let description = input.description.clone().unwrap_or_default();
    let task_type = input.task_type.clone();
    let schedule = input.schedule.clone();
    let script = input.script.clone();
    let notify_channel = input.notify_channel.clone();

    let conn = CRON_DB.lock();
    conn.execute(
        "INSERT INTO cron_tasks (id, name, description, task_type, schedule, script, status, notify_channel, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, ?8, ?9)",
        params![
            id,
            input.name,
            input.description.unwrap_or_default(),
            input.task_type,
            input.schedule,
            input.script,
            input.notify_channel,
            now,
            now,
        ],
    )
    .map_err(|e| format!("Failed to create task: {}", e))?;

    info!("Created cron task: {} ({})", name, id);

    Ok(CronTask {
        id,
        name,
        description,
        task_type,
        schedule,
        script,
        status: "active".to_string(),
        notify_channel,
        created_at: now.clone(),
        updated_at: now,
        last_run: None,
        last_result: None,
        next_run,
    })
}

pub fn list_tasks() -> Result<Vec<CronTask>, String> {
    let conn = CRON_DB.lock();
    let mut stmt = conn
        .prepare(
            "SELECT id, name, description, task_type, schedule, script, status, notify_channel,
                    created_at, updated_at, last_run, last_result
             FROM cron_tasks ORDER BY created_at DESC",
        )
        .map_err(|e| format!("Failed to query tasks: {}", e))?;

    let tasks = stmt
        .query_map([], |row| {
            let schedule: Option<String> = row.get(4)?;
            let next_run = schedule.as_ref().and_then(|s| compute_next_run(s));
            Ok(CronTask {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                task_type: row.get(3)?,
                schedule,
                script: row.get(5)?,
                status: row.get(6)?,
                notify_channel: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
                last_run: row.get(10)?,
                last_result: row.get(11)?,
                next_run,
            })
        })
        .map_err(|e| format!("Failed to map tasks: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect tasks: {}", e))?;

    Ok(tasks)
}

pub fn get_task(id: &str) -> Result<CronTask, String> {
    let conn = CRON_DB.lock();
    let mut stmt = conn
        .prepare(
            "SELECT id, name, description, task_type, schedule, script, status, notify_channel,
                    created_at, updated_at, last_run, last_result
             FROM cron_tasks WHERE id = ?1",
        )
        .map_err(|e| format!("Query error: {}", e))?;

    stmt.query_row(params![id], |row| {
        let schedule: Option<String> = row.get(4)?;
        let next_run = schedule.as_ref().and_then(|s| compute_next_run(s));
        Ok(CronTask {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            task_type: row.get(3)?,
            schedule,
            script: row.get(5)?,
            status: row.get(6)?,
            notify_channel: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            last_run: row.get(10)?,
            last_result: row.get(11)?,
            next_run,
        })
    })
    .map_err(|e| format!("Task not found: {}", e))
}

pub fn update_task(id: &str, input: UpdateTaskInput) -> Result<CronTask, String> {
    // Validate cron expression if being updated
    if let Some(ref schedule) = input.schedule {
        if !schedule.is_empty() {
            validate_cron_expr(schedule)?;
        }
    }

    let now = Utc::now().to_rfc3339();
    let conn = CRON_DB.lock();

    // Build dynamic SET clause
    let mut sets: Vec<String> = vec!["updated_at = ?1".to_string()];
    let mut param_idx = 2u32;
    let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.clone())];

    if let Some(ref name) = input.name {
        sets.push(format!("name = ?{}", param_idx));
        param_values.push(Box::new(name.clone()));
        param_idx += 1;
    }
    if let Some(ref desc) = input.description {
        sets.push(format!("description = ?{}", param_idx));
        param_values.push(Box::new(desc.clone()));
        param_idx += 1;
    }
    if let Some(ref schedule) = input.schedule {
        sets.push(format!("schedule = ?{}", param_idx));
        param_values.push(Box::new(schedule.clone()));
        param_idx += 1;
    }
    if let Some(ref script) = input.script {
        sets.push(format!("script = ?{}", param_idx));
        param_values.push(Box::new(script.clone()));
        param_idx += 1;
    }
    if let Some(ref status) = input.status {
        sets.push(format!("status = ?{}", param_idx));
        param_values.push(Box::new(status.clone()));
        param_idx += 1;
    }
    if let Some(ref notify) = input.notify_channel {
        let val: Option<String> = match notify {
            Value::String(s) => Some(s.clone()),
            Value::Null => None,
            _ => None,
        };
        sets.push(format!("notify_channel = ?{}", param_idx));
        param_values.push(Box::new(val));
        param_idx += 1;
    }

    let _ = param_idx; // suppress unused warning

    let sql = format!("UPDATE cron_tasks SET {} WHERE id = ?{}", sets.join(", "), param_values.len() + 1);
    param_values.push(Box::new(id.to_string()));

    let params_refs: Vec<&dyn rusqlite::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, params_refs.as_slice())
        .map_err(|e| format!("Failed to update task: {}", e))?;

    drop(conn);
    get_task(id)
}

pub fn delete_task(id: &str) -> Result<(), String> {
    let conn = CRON_DB.lock();
    conn.execute("DELETE FROM cron_runs WHERE task_id = ?1", params![id])
        .map_err(|e| format!("Failed to delete task runs: {}", e))?;
    conn.execute("DELETE FROM cron_tasks WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete task: {}", e))?;
    info!("Deleted cron task: {}", id);
    Ok(())
}

// ============================================================================
// Task Execution
// ============================================================================

/// Record a run starting.
fn start_run(task_id: &str) -> Result<i64, String> {
    let conn = CRON_DB.lock();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO cron_runs (task_id, started_at, result) VALUES (?1, ?2, 'running')",
        params![task_id, now],
    )
    .map_err(|e| format!("Failed to start run: {}", e))?;
    Ok(conn.last_insert_rowid())
}

/// Finish a run.
fn finish_run(run_id: i64, result: &str, output: &str) -> Result<(), String> {
    let conn = CRON_DB.lock();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE cron_runs SET finished_at = ?1, result = ?2, output = ?3 WHERE id = ?4",
        params![now, result, output, run_id],
    )
    .map_err(|e| format!("Failed to finish run: {}", e))?;
    Ok(())
}

/// Update task last_run and last_result.
fn update_task_run_status(task_id: &str, result: &str) -> Result<(), String> {
    let conn = CRON_DB.lock();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE cron_tasks SET last_run = ?1, last_result = ?2, updated_at = ?1 WHERE id = ?3",
        params![now, result, task_id],
    )
    .map_err(|e| format!("Failed to update task run status: {}", e))?;
    Ok(())
}

/// Get run history for a task.
pub fn get_runs(task_id: &str, limit: i64) -> Result<Vec<CronRun>, String> {
    let conn = CRON_DB.lock();
    let mut stmt = conn
        .prepare(
            "SELECT id, task_id, started_at, finished_at, result, output
             FROM cron_runs WHERE task_id = ?1 ORDER BY started_at DESC LIMIT ?2",
        )
        .map_err(|e| format!("Query error: {}", e))?;

    let runs = stmt
        .query_map(params![task_id, limit], |row| {
            Ok(CronRun {
                id: row.get(0)?,
                task_id: row.get(1)?,
                started_at: row.get(2)?,
                finished_at: row.get(3)?,
                result: row.get(4)?,
                output: row.get(5)?,
            })
        })
        .map_err(|e| format!("Map error: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Collect error: {}", e))?;

    Ok(runs)
}

/// Execute a task (shell command).
pub async fn execute_task(task_id: &str) -> Result<CronRun, String> {
    let task = get_task(task_id)?;
    let script = task.script.unwrap_or_default();

    if script.is_empty() {
        return Err("Task has no script to execute".to_string());
    }

    let run_id = start_run(task_id)?;
    info!("Executing cron task '{}' (run {})", task.name, run_id);

    // Execute as shell command
    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(&script)
        .output()
        .await
        .map_err(|e| format!("Failed to execute: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = if stderr.is_empty() {
        stdout.clone()
    } else {
        format!("{}\n[stderr]\n{}", stdout, stderr)
    };

    let result = if output.status.success() {
        "success"
    } else {
        "error"
    };

    finish_run(run_id, result, &combined)?;
    update_task_run_status(task_id, result)?;

    // Send notification if configured
    if let Some(ref channel) = task.notify_channel {
        let title = format!(
            "⏰ 定时任务「{}」执行{}",
            task.name,
            if result == "success" { "成功 ✅" } else { "失败 ❌" }
        );
        let body = if combined.len() > 500 {
            format!("{}...", &combined[..500])
        } else {
            combined.clone()
        };
        if let Err(e) = crate::modules::notifications::send_notification(channel, &title, &body).await {
            warn!("Failed to send notification: {}", e);
        }
    }

    // Return the run info
    Ok(CronRun {
        id: run_id,
        task_id: task_id.to_string(),
        started_at: Utc::now().to_rfc3339(),
        finished_at: Some(Utc::now().to_rfc3339()),
        result: result.to_string(),
        output: combined,
    })
}

// ============================================================================
// Background Cron Scheduler
// ============================================================================

/// A state map tracking last check times per task to avoid double-firing.
static LAST_FIRE: Lazy<Mutex<HashMap<String, DateTime<Utc>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Start the background scheduler loop. Call once at app setup.
pub fn start_cron_scheduler() {
    tauri::async_runtime::spawn(async move {
        info!("Cron scheduler started");

        // Check every 30 seconds for due tasks
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

        loop {
            interval.tick().await;

            let tasks = match list_tasks() {
                Ok(t) => t,
                Err(e) => {
                    error!("Cron scheduler: failed to list tasks: {}", e);
                    continue;
                }
            };

            let now = Utc::now();

            for task in tasks {
                if task.status != "active" || task.task_type != "cron" {
                    continue;
                }

                let schedule_str = match task.schedule.as_ref() {
                    Some(s) if !s.is_empty() => s.clone(),
                    _ => continue,
                };

                // Check if task is due
                let normalized = normalize_cron_expr(&schedule_str);
                let schedule = match Schedule::from_str(&normalized) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                // Find the most recent past trigger time
                let prev = schedule.after(&(now - chrono::Duration::seconds(60))).next();

                if let Some(trigger_time) = prev {
                    // Only fire if within the last 60 seconds and haven't fired already
                    if trigger_time <= now && (now - trigger_time).num_seconds() < 60 {
                        let mut last_fires = LAST_FIRE.lock();
                        let should_fire = last_fires
                            .get(&task.id)
                            .map(|last| (now - *last).num_seconds() > 55)
                            .unwrap_or(true);

                        if should_fire {
                            last_fires.insert(task.id.clone(), now);
                            drop(last_fires);

                            let task_id = task.id.clone();
                            let task_name = task.name.clone();
                            tokio::spawn(async move {
                                info!("Cron scheduler firing task: {}", task_name);
                                if let Err(e) = execute_task(&task_id).await {
                                    error!("Cron task '{}' failed: {}", task_name, e);
                                }
                            });
                        }
                    }
                }
            }
        }
    });
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn cron_list_tasks() -> Result<Vec<CronTask>, String> {
    list_tasks()
}

#[tauri::command]
pub async fn cron_create_task(input: CreateTaskInput) -> Result<CronTask, String> {
    create_task(input)
}

#[tauri::command]
pub async fn cron_update_task(id: String, input: UpdateTaskInput) -> Result<CronTask, String> {
    update_task(&id, input)
}

#[tauri::command]
pub async fn cron_delete_task(id: String) -> Result<(), String> {
    delete_task(&id)
}

#[tauri::command]
pub async fn cron_run_task(id: String) -> Result<CronRun, String> {
    execute_task(&id).await
}

#[tauri::command]
pub async fn cron_get_runs(task_id: String, limit: Option<i64>) -> Result<Vec<CronRun>, String> {
    get_runs(&task_id, limit.unwrap_or(20))
}

#[tauri::command]
pub async fn cron_validate_expr(expr: String) -> Result<Value, String> {
    match validate_cron_expr(&expr) {
        Ok(()) => {
            let next = compute_next_run(&expr);
            Ok(serde_json::json!({
                "valid": true,
                "next_run": next,
            }))
        }
        Err(e) => Ok(serde_json::json!({
            "valid": false,
            "error": e,
        })),
    }
}

// ============================================================================
// Heartbeat System (Inspired by CoPaw's HEARTBEAT.md)
// ============================================================================

/// Default heartbeat interval in seconds (30 minutes)
const HEARTBEAT_INTERVAL_SECS: u64 = 30 * 60;

/// Check if heartbeat is configured (HEARTBEAT.md exists in ~/.helix/)
fn load_heartbeat_config() -> Option<String> {
    let helix_dir = dirs::home_dir()?.join(".helix");
    let heartbeat_path = helix_dir.join("HEARTBEAT.md");
    std::fs::read_to_string(&heartbeat_path).ok()
}

/// Start the heartbeat loop. Reads ~/.helix/HEARTBEAT.md periodically
/// and sends its content as a prompt to the agent.
pub fn start_heartbeat() {
    tauri::async_runtime::spawn(async move {
        // Wait 60 seconds after startup before first heartbeat
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs(HEARTBEAT_INTERVAL_SECS)
        );

        info!("Heartbeat system started (interval: {}s)", HEARTBEAT_INTERVAL_SECS);

        loop {
            interval.tick().await;

            // Check if HEARTBEAT.md exists
            let heartbeat_content = match load_heartbeat_config() {
                Some(content) if !content.trim().is_empty() => content,
                _ => continue, // No heartbeat config, skip
            };

            // Check active hours (default: 8:00 - 23:00)
            let hour = chrono::Local::now().hour();
            if !(8..=23).contains(&hour) {
                continue; // Outside active hours
            }

            info!("[heartbeat] Executing heartbeat check");

            // Build heartbeat prompt
            let prompt = format!(
                "[HEARTBEAT] {}\n\n{}\n\nIf nothing needs attention, respond with HEARTBEAT_OK.",
                chrono::Local::now().format("%Y-%m-%d %H:%M"),
                heartbeat_content.trim()
            );

            // Run through the agent
            match crate::modules::agent::agent_process_message(
                "heartbeat",
                &prompt,
                None,
            ).await {
                Ok(response) => {
                    if response.trim() != "HEARTBEAT_OK" && !response.is_empty() {
                        info!("[heartbeat] Agent response: {}", &response[..response.len().min(200)]);
                        // Emit heartbeat result to frontend
                        crate::modules::agent::emit_agent_progress(
                            "heartbeat",
                            serde_json::json!({ "response": response }),
                        );
                    }
                }
                Err(e) => {
                    warn!("[heartbeat] Agent error: {}", e);
                }
            }
        }
    });
}
