//! Agent Tools — Direct agents-sdk tool definitions.
//!
//! Each tool is created via `agents_sdk::tool()` with its schema and handler.
//! No intermediate JSON schema layer or dispatcher needed.

use anyhow;

use tracing::info;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use agents_sdk::{ToolResult, ToolContext, ToolParameterSchema};

/// Tracks files sent in the current agent session (metadata for response).
static SENT_FILES: std::sync::LazyLock<Mutex<Vec<Value>>> =
    std::sync::LazyLock::new(|| Mutex::new(Vec::new()));

/// Call at the start of each agent call to reset sent-file tracking.
pub fn clear_sent_files() {
    if let Ok(mut files) = SENT_FILES.lock() {
        files.clear();
    }
}

/// Get and clear sent files metadata (called by agent_chat to include in response)
pub fn take_sent_files() -> Vec<Value> {
    if let Ok(mut files) = SENT_FILES.lock() {
        std::mem::take(&mut *files)
    } else {
        Vec::new()
    }
}

/// Sandbox directory for agent file writes — all file_write/file_edit operations
/// are restricted to this directory to prevent the agent from writing files everywhere.
const SANDBOX_DIR: &str = "helix_workspace";

/// Get the full sandbox directory path
fn get_sandbox_path() -> String {
    if let Some(home) = dirs::home_dir() {
        format!("{}/{}", home.display(), SANDBOX_DIR)
    } else {
        format!("./{}", SANDBOX_DIR)
    }
}

/// Validate that a path is within the sandbox directory.
/// Returns the canonicalized path if valid, or an error message.
fn validate_sandbox_path(path: &str) -> Result<String, String> {
    let sandbox = get_sandbox_path();
    let _ = std::fs::create_dir_all(&sandbox);
    
    let expanded = expand_path(path);
    let abs_path = if std::path::Path::new(&expanded).is_absolute() {
        expanded
    } else {
        format!("{}/{}", sandbox, expanded)
    };
    
    let canonical_sandbox = std::fs::canonicalize(&sandbox)
        .unwrap_or_else(|_| std::path::PathBuf::from(&sandbox));
    
    let path_buf = std::path::PathBuf::from(&abs_path);
    let check_path = if path_buf.exists() {
        std::fs::canonicalize(&abs_path)
            .unwrap_or_else(|_| path_buf.clone())
    } else {
        if let Some(parent) = path_buf.parent() {
            let _ = std::fs::create_dir_all(parent);
            let resolved_parent = std::fs::canonicalize(parent)
                .unwrap_or_else(|_| parent.to_path_buf());
            resolved_parent.join(path_buf.file_name().unwrap_or_default())
        } else {
            path_buf
        }
    };
    
    if check_path.starts_with(&canonical_sandbox) {
        Ok(abs_path)
    } else {
        Err(format!(
            "❌ 安全限制: 只能在 ~/{} 目录下写入文件。请使用该目录下的路径。\n当前路径: {}",
            SANDBOX_DIR, abs_path
        ))
    }
}


// ============================================================================
// Legacy Types — kept for plugins.rs manifest parsing
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub r#type: String,
    pub function: ToolFunctionDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

// ============================================================================
// Schema Helpers
// ============================================================================

fn param(name: &str, ptype: &str, desc: Option<&str>) -> (String, ToolParameterSchema) {
    (name.to_string(), ToolParameterSchema {
        schema_type: ptype.to_string(),
        description: desc.map(String::from),
        properties: None,
        required: None,
        items: None,
        enum_values: None,
        default: None,
        additional: Default::default(),
    })
}

fn schema(props: Vec<(String, ToolParameterSchema)>, required: Vec<&str>) -> ToolParameterSchema {
    let mut map = std::collections::HashMap::new();
    for (k, v) in props {
        map.insert(k, v);
    }
    ToolParameterSchema {
        schema_type: "object".to_string(),
        description: None,
        properties: Some(map),
        required: Some(required.into_iter().map(String::from).collect()),
        items: None,
        enum_values: None,
        default: None,
        additional: Default::default(),
    }
}

// ============================================================================
// Build All Tools — returns Vec<Arc<dyn Tool>> for agents-sdk
// ============================================================================

pub fn build_tools() -> Vec<Arc<dyn agents_sdk::Tool>> {
    vec![
        agents_sdk::tool(
            "shell_exec",
            "Execute a shell command on the system and return stdout/stderr.",
            schema(vec![
                param("command", "string", Some("The shell command to execute")),
                param("working_dir", "string", Some("Working directory (default: home)")),
                param("timeout_secs", "integer", Some("Timeout in seconds (default: 30)")),
            ], vec!["command"]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_shell_exec(&args).await.map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "file_read",
            "Read the contents of a file.",
            schema(vec![
                param("path", "string", Some("Path to the file to read")),
                param("max_lines", "integer", Some("Max lines to return (default: 500)")),
            ], vec!["path"]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_file_read(&args).await.map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "file_write",
            &format!("Write content to a file. RESTRICTED: files can only be written inside ~/{}/", SANDBOX_DIR),
            schema(vec![
                param("path", "string", Some(&format!("File path (relative = inside ~/{}/)", SANDBOX_DIR))),
                param("content", "string", Some("Content to write")),
                param("append", "boolean", Some("Append instead of overwrite")),
            ], vec!["path", "content"]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_file_write(&args).await.map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "file_edit",
            &format!("Edit a file by replacing text. RESTRICTED: only files inside ~/{}/", SANDBOX_DIR),
            schema(vec![
                param("path", "string", None),
                param("search", "string", Some("Text to find")),
                param("replace", "string", Some("Replacement text")),
                param("all", "boolean", Some("Replace all occurrences")),
            ], vec!["path", "search", "replace"]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_file_edit(&args).await.map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "web_fetch",
            "Fetch content from a URL. Supports GET/POST/PUT/DELETE with custom headers.",
            schema(vec![
                param("url", "string", Some("URL to fetch")),
                param("method", "string", Some("HTTP method (default: GET)")),
                param("headers", "object", Some("Custom headers")),
                param("body", "string", Some("Request body")),
            ], vec!["url"]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_web_fetch(&args).await.map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "web_search",
            "Search the web. Also handles weather queries and hot searches.",
            schema(vec![
                param("query", "string", Some("Search query")),
                param("num_results", "integer", Some("Number of results (default: 5)")),
            ], vec!["query"]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_web_search(&args).await.map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "memory_store",
            "Store information in long-term memory with a key.",
            schema(vec![
                param("key", "string", Some("Memory key")),
                param("value", "string", Some("Content to store")),
            ], vec!["key", "value"]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_memory_store(&args).await.map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "memory_recall",
            "Recall stored information from long-term memory.",
            schema(vec![
                param("query", "string", Some("Search query for memories")),
            ], vec!["query"]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_memory_recall(&args).await.map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "list_dir",
            "List files and directories in a given path.",
            schema(vec![
                param("path", "string", Some("Directory path")),
                param("recursive", "boolean", Some("List recursively")),
                param("max_depth", "integer", Some("Max directory depth (default: 1)")),
            ], vec!["path"]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_list_dir(&args).map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "grep_search",
            "Search for a text pattern in files using grep.",
            schema(vec![
                param("pattern", "string", Some("Text pattern to search")),
                param("path", "string", Some("Directory or file to search in")),
                param("include", "string", Some("File glob filter (e.g. *.rs)")),
                param("ignore_case", "boolean", None),
                param("max_results", "integer", None),
            ], vec!["pattern", "path"]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_grep_search(&args).await.map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "find_files",
            "Find files and directories by name pattern.",
            schema(vec![
                param("path", "string", Some("Root directory to search")),
                param("name", "string", Some("File name pattern (e.g. *.pdf)")),
                param("max_depth", "integer", None),
                param("max_results", "integer", None),
            ], vec!["path"]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_find_files(&args).await.map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "process_list",
            "List running processes with CPU/memory usage.",
            schema(vec![
                param("filter", "string", Some("Filter by process name")),
                param("limit", "integer", Some("Max number of results")),
            ], vec![]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_process_list(&args).await.map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "process_kill",
            "Kill a process by PID or name.",
            schema(vec![
                param("pid", "integer", Some("Process ID to kill")),
                param("name", "string", Some("Process name to kill")),
                param("signal", "string", Some("Signal (default: TERM)")),
            ], vec![]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_process_kill(&args).await.map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "sysinfo",
            "Get system information: OS, CPU, memory, disk, uptime.",
            schema(vec![], vec![]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_sysinfo(&args).map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
        agents_sdk::tool(
            "chat_send_file",
            "Send a file to the user as a downloadable attachment in the chat.",
            schema(vec![
                param("path", "string", Some("Absolute path to the file to send")),
            ], vec!["path"]),
            |args: Value, ctx: ToolContext| async move {
                let r = tool_chat_send_file(&args).await.map_err(anyhow::Error::msg)?;
                Ok(ToolResult::text(&ctx, r))
            },
        ),
    ]
}

// ============================================================================
// Backward-compat dispatcher — used by subagents.rs and api_server.rs
// ============================================================================

pub async fn execute_tool(name: &str, args: &Value, _ctx: Option<&str>) -> Result<String, String> {
    match name {
        "shell_exec"     => tool_shell_exec(args).await,
        "file_read"      => tool_file_read(args).await,
        "file_write"     => tool_file_write(args).await,
        "file_edit"      => tool_file_edit(args).await,
        "web_fetch"      => tool_web_fetch(args).await,
        "web_search"     => tool_web_search(args).await,
        "memory_store"   => tool_memory_store(args).await,
        "memory_recall"  => tool_memory_recall(args).await,
        "list_dir"       => tool_list_dir(args),
        "grep_search"    => tool_grep_search(args).await,
        "find_files"     => tool_find_files(args).await,
        "process_list"   => tool_process_list(args).await,
        "process_kill"   => tool_process_kill(args).await,
        "sysinfo"        => tool_sysinfo(args),
        "chat_send_file" => tool_chat_send_file(args).await,
        other => Err(format!("Unknown tool: {}", other)),
    }
}


// ============================================================================
// Tool Implementations
// ============================================================================

fn expand_path(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{}", home.display(), &path[2..]);
        }
    }
    path.to_string()
}

// ---- Shell Exec ----
async fn tool_shell_exec(args: &Value) -> Result<String, String> {
    let cmd = args["command"].as_str().ok_or("Missing 'command'")?;
    let working_dir = args["working_dir"]
        .as_str()
        .map(|s| expand_path(s))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string())
        });
    let timeout = args["timeout_secs"].as_u64().unwrap_or(30);

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(timeout),
        if cfg!(target_os = "macos") {
            tokio::process::Command::new("zsh")
                .arg("-l")
                .arg("-c")
                .arg(cmd)
                .current_dir(&working_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
        } else {
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(&working_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
        },
    )
    .await
    .map_err(|_| format!("Command timed out after {}s", timeout))?
    .map_err(|e| format!("Command failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let code = output.status.code().unwrap_or(-1);

    let max = 8000;
    let stdout_trunc = if stdout.len() > max { &stdout[..max] } else { &stdout };
    let stderr_trunc = if stderr.len() > max { &stderr[..max] } else { &stderr };

    Ok(format!(
        "Exit code: {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        code, stdout_trunc, stderr_trunc
    ))
}

// ---- File Read ----
async fn tool_file_read(args: &Value) -> Result<String, String> {
    let path = expand_path(args["path"].as_str().ok_or("Missing 'path'")?);
    let max_lines = args["max_lines"].as_u64().unwrap_or(500) as usize;

    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Read '{}': {}", path, e))?;

    let lines: Vec<&str> = content.lines().collect();
    if lines.len() > max_lines {
        Ok(format!(
            "{}\n\n... ({} more lines, total {})",
            lines[..max_lines].join("\n"),
            lines.len() - max_lines,
            lines.len()
        ))
    } else {
        Ok(content)
    }
}

// ---- Chat Send File ----
async fn tool_chat_send_file(args: &Value) -> Result<String, String> {
    let path = expand_path(args["path"].as_str().ok_or("Missing 'path'")?);
    let display_name = args["display_name"]
        .as_str()
        .unwrap_or_else(|| {
            std::path::Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
        })
        .to_string();

    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| format!("Cannot access '{}': {}", path, e))?;

    let size_bytes = meta.len();
    let size_str = if size_bytes > 1024 * 1024 {
        format!("{:.1} MB", size_bytes as f64 / 1024.0 / 1024.0)
    } else if size_bytes > 1024 {
        format!("{:.1} KB", size_bytes as f64 / 1024.0)
    } else {
        format!("{} B", size_bytes)
    };

    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mime = match ext.as_str() {
        "pdf"        => "application/pdf",
        "png"        => "image/png",
        "jpg"|"jpeg" => "image/jpeg",
        "gif"        => "image/gif",
        "webp"       => "image/webp",
        "zip"        => "application/zip",
        "tar"|"gz"   => "application/gzip",
        "txt"|"md"   => "text/plain",
        "json"       => "application/json",
        "csv"        => "text/csv",
        "mp3"        => "audio/mpeg",
        "mp4"        => "video/mp4",
        "docx"       => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx"       => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _            => "application/octet-stream",
    };

    if let Ok(files) = SENT_FILES.lock() {
        if files.iter().any(|f| f["path"].as_str() == Some(&path)) {
            return Ok(format!("文件「{}」已经发送过了，无需重复发送。", display_name));
        }
    }

    let file_meta = json!({
        "name": display_name,
        "path": path,
        "mime": mime,
        "size": size_str,
    });
    info!("[chat_send_file] Stored file metadata: name={}, path={}", display_name, path);
    if let Ok(mut files) = SENT_FILES.lock() {
        files.push(file_meta);
    }

    Ok(format!("✅ 文件「{}」({})已发送到对话框，用户可以点击「另存为」下载。", display_name, size_str))
}


// ---- File Write ----
async fn tool_file_write(args: &Value) -> Result<String, String> {
    let raw_path = args["path"].as_str().ok_or("Missing 'path'")?;
    let path = validate_sandbox_path(raw_path)?;
    let content = args["content"].as_str().ok_or("Missing 'content'")?;
    let append = args["append"].as_bool().unwrap_or(false);

    if let Some(parent) = std::path::Path::new(&path).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("mkdir: {}", e))?;
    }

    if append {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|e| format!("Open '{}': {}", path, e))?;
        file.write_all(content.as_bytes())
            .await
            .map_err(|e| format!("Write: {}", e))?;
    } else {
        tokio::fs::write(&path, content)
            .await
            .map_err(|e| format!("Write '{}': {}", path, e))?;
    }

    Ok(format!("✅ Written {} bytes to {}", content.len(), path))
}

// ---- File Edit ----
async fn tool_file_edit(args: &Value) -> Result<String, String> {
    let raw_path = args["path"].as_str().ok_or("Missing 'path'")?;
    let path = validate_sandbox_path(raw_path)?;
    let search = args["search"].as_str().ok_or("Missing 'search'")?;
    let replace = args["replace"].as_str().ok_or("Missing 'replace'")?;
    let all = args["all"].as_bool().unwrap_or(false);

    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Read '{}': {}", path, e))?;

    let count = content.matches(search).count();
    if count == 0 {
        return Err(format!("Text not found in {}", path));
    }

    let new_content = if all {
        content.replace(search, replace)
    } else {
        content.replacen(search, replace, 1)
    };

    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| format!("Write '{}': {}", path, e))?;

    Ok(format!("✅ Replaced {} occurrence(s) in {}", if all { count } else { 1 }, path))
}

// ---- Web Fetch ----
async fn tool_web_fetch(args: &Value) -> Result<String, String> {
    let url = args["url"].as_str().ok_or("Missing 'url'")?;
    let method = args["method"].as_str().unwrap_or("GET").to_uppercase();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut req = match method.as_str() {
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "DELETE" => client.delete(url),
        _ => client.get(url),
    };

    if let Some(headers) = args["headers"].as_object() {
        for (k, v) in headers {
            if let Some(val) = v.as_str() {
                req = req.header(k.as_str(), val);
            }
        }
    }

    if let Some(body) = args["body"].as_str() {
        req = req.body(body.to_string());
    }

    let resp = req.send().await.map_err(|e| format!("Fetch: {}", e))?;
    let status = resp.status().as_u16();
    let body = resp.text().await.map_err(|e| format!("Read: {}", e))?;

    let max = 15000;
    let truncated = body.len() > max;
    let text = if truncated { &body[..max] } else { &body };

    Ok(format!(
        "Status: {}\n{}\n{}",
        status,
        text,
        if truncated { format!("\n... (truncated, {} total bytes)", body.len()) } else { String::new() }
    ))
}

// ---- Web Search (multi-engine) ----
async fn tool_web_search(args: &Value) -> Result<String, String> {
    let query = args["query"].as_str().ok_or("Missing 'query'")?;
    let num = args["num_results"].as_u64().unwrap_or(5) as usize;
    let query_lower = query.to_lowercase();

    // Weather shortcut
    let weather_keywords = ["天气", "weather", "温度", "气温"];
    if weather_keywords.iter().any(|k| query_lower.contains(k)) {
        let loc = query_lower
            .replace("天气", "").replace("weather", "")
            .replace("温度", "").replace("气温", "")
            .replace("怎么样", "").replace("查询", "")
            .trim().to_string();
        let loc = if loc.is_empty() { "Beijing".to_string() } else { loc };
        let url = format!("https://wttr.in/{}?format=4&lang=zh", loc);
        if let Ok(resp) = reqwest::get(&url).await {
            if let Ok(text) = resp.text().await {
                if !text.is_empty() && !text.contains("Unknown") {
                    return Ok(format!("🌤 {}", text.trim()));
                }
            }
        }
    }

    // Hot search shortcut
    let hot_keywords = ["热搜", "热榜", "热门", "热点", "trending", "热议", "新闻", "头条"];
    if hot_keywords.iter().any(|k| query_lower.contains(k)) {
        if let Ok(result) = fetch_baidu_hot().await {
            if !result.is_empty() {
                return Ok(result);
            }
        }
    }

    // General search: DuckDuckGo → Bing → Baidu
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    if let Ok(results) = search_duckduckgo(&client, query, num).await {
        if !results.is_empty() { return Ok(results); }
    }
    if let Ok(results) = search_bing(&client, query, num).await {
        if !results.is_empty() { return Ok(results); }
    }
    if let Ok(results) = search_baidu(&client, query, num).await {
        if !results.is_empty() { return Ok(results); }
    }

    Ok(format!("搜索 '{}' 未找到结果。建议使用 web_fetch 工具直接访问目标网站。", query))
}

// ---- Baidu Hot Search ----
async fn fetch_baidu_hot() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("Mozilla/5.0")
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let resp = client
        .get("https://top.baidu.com/api/board?platform=wise&tab=realtime")
        .header("Accept", "application/json")
        .send().await.map_err(|e| format!("Baidu: {}", e))?;

    let body = resp.text().await.map_err(|e| format!("Read: {}", e))?;

    if let Ok(data) = serde_json::from_str::<Value>(&body) {
        let mut results = Vec::new();
        if let Some(cards) = data["data"]["cards"].as_array() {
            for card in cards {
                if let Some(content_arr) = card["content"].as_array() {
                    for content_item in content_arr {
                        if let Some(inner_items) = content_item["content"].as_array() {
                            for item in inner_items.iter().take(20) {
                                let word = item["word"].as_str().unwrap_or("");
                                if !word.is_empty() && results.len() < 15 {
                                    results.push(format!("{}. {}", results.len() + 1, word));
                                }
                            }
                        } else if let Some(word) = content_item["word"].as_str() {
                            if !word.is_empty() && results.len() < 15 {
                                results.push(format!("{}. {}", results.len() + 1, word));
                            }
                        }
                    }
                }
            }
        }
        if !results.is_empty() {
            return Ok(format!("📊 百度实时热搜榜:\n\n{}", results.join("\n")));
        }
    }
    Err("Failed to parse Baidu hot search".to_string())
}

// ---- DuckDuckGo Search ----
async fn search_duckduckgo(client: &reqwest::Client, query: &str, num: usize) -> Result<String, String> {
    let resp = client
        .get("https://html.duckduckgo.com/html/")
        .query(&[("q", query)])
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .send().await.map_err(|e| format!("DDG: {}", e))?;

    let body = resp.text().await.map_err(|e| format!("Read: {}", e))?;
    let mut results = Vec::new();

    for chunk in body.split("result__a") {
        if results.len() >= num { break; }
        if let Some(href_start) = chunk.find("href=\"") {
            let rest = &chunk[href_start + 6..];
            if let Some(href_end) = rest.find('"') {
                let url = &rest[..href_end];
                if !url.starts_with("http") && !url.contains("duckduckgo.com/l/") { continue; }

                let real_url = if url.contains("uddg=") {
                    if let Some(start) = url.find("uddg=") {
                        let encoded = &url[start + 5..];
                        let end = encoded.find('&').unwrap_or(encoded.len());
                        percent_decode(&encoded[..end])
                    } else { url.to_string() }
                } else { url.to_string() };

                let title = extract_text_between(rest, '>', '<');
                if !title.is_empty() && title.len() > 2 {
                    results.push(format!("{}. {} — {}", results.len() + 1, title, real_url));
                }
            }
        }
    }

    if results.is_empty() { Err("No DDG results".into()) } else { Ok(results.join("\n\n")) }
}

// ---- Bing Search ----
async fn search_bing(client: &reqwest::Client, query: &str, num: usize) -> Result<String, String> {
    let resp = client
        .get("https://www.bing.com/search")
        .query(&[("q", query), ("setlang", "zh-Hans")])
        .header("Accept-Language", "zh-CN,zh;q=0.9")
        .send().await.map_err(|e| format!("Bing: {}", e))?;

    let body = resp.text().await.map_err(|e| format!("Read: {}", e))?;
    let mut results = Vec::new();

    for chunk in body.split("class=\"b_algo\"") {
        if results.len() >= num { break; }
        if let Some(href_start) = chunk.find("href=\"") {
            let rest = &chunk[href_start + 6..];
            if let Some(href_end) = rest.find('"') {
                let url = &rest[..href_end];
                if !url.starts_with("http") { continue; }
                let title = extract_text_between(rest, '>', '<');
                if !title.is_empty() {
                    results.push(format!("{}. {} — {}", results.len() + 1, title, url));
                }
            }
        }
    }

    if results.is_empty() { Err("No Bing results".into()) } else { Ok(results.join("\n\n")) }
}

// ---- Baidu Search ----
async fn search_baidu(client: &reqwest::Client, query: &str, num: usize) -> Result<String, String> {
    let resp = client
        .get("https://www.baidu.com/s")
        .query(&[("wd", query)])
        .header("Accept-Language", "zh-CN,zh;q=0.9")
        .send().await.map_err(|e| format!("Baidu: {}", e))?;

    let body = resp.text().await.map_err(|e| format!("Read: {}", e))?;
    let mut results = Vec::new();

    for chunk in body.split("class=\"c-container\"") {
        if results.len() >= num { break; }
        if let Some(href_start) = chunk.find("href=\"") {
            let rest = &chunk[href_start + 6..];
            if let Some(href_end) = rest.find('"') {
                let url = &rest[..href_end];
                if !url.starts_with("http") { continue; }
                let title = extract_text_between(rest, '>', '<');
                if !title.is_empty() && title.len() > 3 {
                    results.push(format!("{}. {} — {}", results.len() + 1, title, url));
                }
            }
        }
    }

    if results.is_empty() { Err("No Baidu results".into()) } else { Ok(results.join("\n\n")) }
}

// ---- Memory Store ----
async fn tool_memory_store(args: &Value) -> Result<String, String> {
    let key = args["key"].as_str().ok_or("Missing 'key'")?;
    let value = args["value"].as_str().ok_or("Missing 'value'")?;
    super::memory::memory_store_entry(key.to_string(), value.to_string(), None, None).await?;
    Ok(format!("✅ Stored under key '{}'", key))
}

// ---- Memory Recall ----
async fn tool_memory_recall(args: &Value) -> Result<String, String> {
    let query = args["query"].as_str().ok_or("Missing 'query'")?;
    let results = super::memory::memory_search(query.to_string(), Some(10)).await?;
    if results.is_empty() {
        Ok("No matching memories found.".to_string())
    } else {
        let mut output = format!("Found {} memories:\n\n", results.len());
        for r in &results {
            output.push_str(&format!("**{}**: {}\n\n", r.entry.key, r.entry.content));
        }
        Ok(output)
    }
}

// ---- List Dir ----
fn tool_list_dir(args: &Value) -> Result<String, String> {
    let path = expand_path(args["path"].as_str().ok_or("Missing 'path'")?);
    let recursive = args["recursive"].as_bool().unwrap_or(false);
    let max_depth = args["max_depth"].as_u64().unwrap_or(1) as usize;

    let mut entries = Vec::new();
    list_dir_recursive(&path, 0, if recursive { max_depth } else { 1 }, &mut entries)?;

    if entries.is_empty() {
        Ok(format!("Directory '{}' is empty.", path))
    } else {
        Ok(entries.join("\n"))
    }
}

fn list_dir_recursive(path: &str, depth: usize, max_depth: usize, entries: &mut Vec<String>) -> Result<(), String> {
    if depth >= max_depth || entries.len() > 500 { return Ok(()); }
    let dir = std::fs::read_dir(path).map_err(|e| format!("Read dir '{}': {}", path, e))?;
    let indent = "  ".repeat(depth);
    for entry in dir {
        if let Ok(entry) = entry {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') { continue; }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let prefix = if is_dir { "📁" } else { "📄" };
            entries.push(format!("{}{} {}", indent, prefix, name));
            if is_dir && depth + 1 < max_depth {
                let _ = list_dir_recursive(&entry.path().to_string_lossy(), depth + 1, max_depth, entries);
            }
        }
    }
    Ok(())
}

// ---- Grep Search ----
async fn tool_grep_search(args: &Value) -> Result<String, String> {
    let pattern = args["pattern"].as_str().ok_or("Missing 'pattern'")?;
    let path = expand_path(args["path"].as_str().ok_or("Missing 'path'")?);
    let ignore_case = args["ignore_case"].as_bool().unwrap_or(false);
    let max_results = args["max_results"].as_u64().unwrap_or(50);
    let include = args["include"].as_str().unwrap_or("");

    let mut cmd_parts = vec!["grep", "-rn"];
    if ignore_case { cmd_parts.push("-i"); }
    let max_flag = format!("-m{}", max_results);
    cmd_parts.push(&max_flag);
    let include_flag;
    if !include.is_empty() {
        include_flag = format!("--include={}", include);
        cmd_parts.push(&include_flag);
    }
    cmd_parts.push(pattern);
    cmd_parts.push(&path);

    let output = tokio::process::Command::new(cmd_parts[0])
        .args(&cmd_parts[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("grep: {}", e))?;

    let result = String::from_utf8_lossy(&output.stdout);
    if result.is_empty() {
        Ok(format!("No matches for '{}' in {}", pattern, path))
    } else {
        let max = 5000;
        Ok(if result.len() > max { result[..max].to_string() } else { result.to_string() })
    }
}

// ---- Find Files ----
async fn tool_find_files(args: &Value) -> Result<String, String> {
    let path = expand_path(args["path"].as_str().ok_or("Missing 'path'")?);
    let name = args["name"].as_str().unwrap_or("*");
    let max_depth = args["max_depth"].as_u64().unwrap_or(5);
    let max_results = args["max_results"].as_u64().unwrap_or(50);

    let output = tokio::process::Command::new("find")
        .arg(&path)
        .arg("-maxdepth").arg(max_depth.to_string())
        .arg("-name").arg(name)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("find: {}", e))?;

    let result = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = result.lines().take(max_results as usize).collect();
    if lines.is_empty() {
        Ok(format!("No files matching '{}' in {}", name, path))
    } else {
        Ok(lines.join("\n"))
    }
}

// ---- Process List ----
async fn tool_process_list(args: &Value) -> Result<String, String> {
    let filter = args["filter"].as_str().unwrap_or("");
    let limit = args["limit"].as_u64().unwrap_or(20) as usize;

    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut procs: Vec<String> = Vec::new();
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_string();
        if !filter.is_empty() && !name.to_lowercase().contains(&filter.to_lowercase()) {
            continue;
        }
        let cpu = process.cpu_usage();
        let mem = process.memory() / 1024 / 1024;
        procs.push(format!("PID {} | {} | CPU: {:.1}% | MEM: {}MB", pid, name, cpu, mem));
        if procs.len() >= limit { break; }
    }

    if procs.is_empty() {
        Ok("No matching processes found.".to_string())
    } else {
        Ok(procs.join("\n"))
    }
}

// ---- Process Kill ----
async fn tool_process_kill(args: &Value) -> Result<String, String> {
    let pid = args["pid"].as_u64();
    let name = args["name"].as_str();
    let signal = args["signal"].as_str().unwrap_or("TERM");

    if let Some(pid) = pid {
        let output = tokio::process::Command::new("kill")
            .arg(format!("-{}", signal))
            .arg(pid.to_string())
            .output()
            .await
            .map_err(|e| format!("kill: {}", e))?;
        if output.status.success() {
            Ok(format!("✅ Killed PID {}", pid))
        } else {
            Err(format!("Failed to kill PID {}: {}", pid, String::from_utf8_lossy(&output.stderr)))
        }
    } else if let Some(name) = name {
        let output = tokio::process::Command::new("pkill")
            .arg(format!("-{}", signal))
            .arg(name)
            .output()
            .await
            .map_err(|e| format!("pkill: {}", e))?;
        if output.status.success() {
            Ok(format!("✅ Killed processes matching '{}'", name))
        } else {
            Err(format!("Failed to kill '{}': {}", name, String::from_utf8_lossy(&output.stderr)))
        }
    } else {
        Err("Must provide 'pid' or 'name'".to_string())
    }
}

// ---- System Info ----
fn tool_sysinfo(_args: &Value) -> Result<String, String> {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_mem = sys.total_memory() / 1024 / 1024;
    let used_mem = sys.used_memory() / 1024 / 1024;
    let cpu_count = sys.cpus().len();
    let os = System::name().unwrap_or_default();
    let os_ver = System::os_version().unwrap_or_default();
    let kernel = System::kernel_version().unwrap_or_default();
    let hostname = System::host_name().unwrap_or_default();
    let uptime = System::uptime();

    Ok(format!(
        "🖥 System Info\n\
         - OS: {} {}\n\
         - Kernel: {}\n\
         - Host: {}\n\
         - CPUs: {}\n\
         - Memory: {} / {} MB ({:.1}%)\n\
         - Uptime: {}h {}m",
        os, os_ver, kernel, hostname, cpu_count,
        used_mem, total_mem, used_mem as f64 / total_mem as f64 * 100.0,
        uptime / 3600, (uptime % 3600) / 60
    ))
}

// ============================================================================
// HTML Parsing Helpers
// ============================================================================

fn extract_text_between(html: &str, open: char, close: char) -> String {
    if let Some(start) = html.find(open) {
        let after = &html[start + open.len_utf8()..];
        if let Some(end) = after.find(close) {
            return after[..end].trim().to_string();
        }
    }
    String::new()
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

fn percent_decode(input: &str) -> String {
    let mut result = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i+1..i+3]).unwrap_or(""), 16,
            ) {
                result.push(val);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' { result.push(b' '); } else { result.push(bytes[i]); }
        i += 1;
    }
    String::from_utf8(result).unwrap_or_else(|_| input.to_string())
}

// ============================================================================
// Legacy Tauri Commands (kept for backward compatibility)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFetchResult {
    pub url: String,
    pub title: Option<String>,
    pub content: String,
    pub content_type: Option<String>,
    pub status: u16,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timed_out: bool,
}

#[tauri::command]
pub async fn tool_web_search_cmd(_query: String, _count: Option<u32>, _api_key: Option<String>) -> Result<Vec<WebSearchResult>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn tool_web_fetch_cmd(url: String, max_chars: Option<usize>) -> Result<WebFetchResult, String> {
    let args = json!({"url": url, "max_chars": max_chars});
    let result = tool_web_fetch(&args).await?;
    Ok(WebFetchResult {
        url,
        title: None,
        content: result,
        content_type: None,
        status: 200,
        truncated: false,
    })
}

#[tauri::command]
pub async fn tool_bash_exec_cmd(command: String, timeout_secs: Option<u64>, cwd: Option<String>) -> Result<BashExecResult, String> {
    let args = json!({"command": command, "timeout_secs": timeout_secs, "working_dir": cwd});
    let result = tool_shell_exec(&args).await?;
    Ok(BashExecResult {
        stdout: result,
        stderr: String::new(),
        exit_code: 0,
        timed_out: false,
    })
}

#[tauri::command]
pub async fn tool_image_describe(image_path: String, prompt: Option<String>) -> Result<String, String> {
    let prompt = prompt.unwrap_or_else(|| "Describe this image in detail.".to_string());
    let bytes = tokio::fs::read(&image_path).await.map_err(|e| format!("read: {}", e))?;

    let mime = if image_path.ends_with(".png") { "image/png" }
        else if image_path.ends_with(".gif") { "image/gif" }
        else if image_path.ends_with(".webp") { "image/webp" }
        else { "image/jpeg" };

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    let config = crate::modules::config::load_app_config().map_err(|e| format!("config: {}", e))?;
    let ai = &config.ai_config;
    if ai.api_key.is_empty() { return Err("API key not configured".to_string()); }

    let url_str = format!("{}/chat/completions", ai.base_url.trim_end_matches('/'));
    let body = json!({
        "model": ai.model,
        "messages": [{"role":"user","content":[
            {"type":"text","text": prompt},
            {"type":"image_url","image_url":{"url": format!("data:{};base64,{}", mime, b64)}}
        ]}],
        "max_tokens": 1024
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build().unwrap_or_else(|_| reqwest::Client::new());

    let resp = client.post(&url_str)
        .header("Authorization", format!("Bearer {}", ai.api_key))
        .json(&body).send().await.map_err(|e| format!("vision: {}", e))?;

    if !resp.status().is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("vision error: {}", &err[..err.len().min(300)]));
    }

    let data: Value = resp.json().await.map_err(|e| format!("parse: {}", e))?;
    Ok(data["choices"][0]["message"]["content"].as_str().unwrap_or("Unable to describe image").to_string())
}
