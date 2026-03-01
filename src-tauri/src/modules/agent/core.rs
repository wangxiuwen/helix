//! Helix Agent — Powered by agents-sdk.
//!
//! Uses ConfigurableAgentBuilder for the agent loop with custom tools.

use agents_sdk::{
    ConfigurableAgentBuilder, OpenAiConfig, OpenAiChatModel,
    state::AgentStateSnapshot,
    persistence::InMemoryCheckpointer,
};

use serde_json::{json, Value};
use tracing::info;
use std::sync::Arc;

use crate::modules::config::load_app_config;
use crate::modules::database;

use std::collections::HashMap;
use std::sync::Mutex as StdMutex;

/// Per-session cancellation flags
static CANCELLED_SESSIONS: std::sync::LazyLock<StdMutex<HashMap<String, bool>>> =
    std::sync::LazyLock::new(|| StdMutex::new(HashMap::new()));

tokio::task_local! {
    /// Per-session workspace directory, accessible from tool closures
    pub static SESSION_WORKSPACE: Option<String>;
    /// Per-session account ID, accessible from tool closures
    pub static SESSION_ACCOUNT_ID: String;
}

/// Cancel a running agent session
#[tauri::command]
pub fn agent_cancel(session_id: Option<String>) {
    if let Ok(mut map) = CANCELLED_SESSIONS.lock() {
        if let Some(sid) = session_id {
            map.insert(sid, true);
        } else {
            // Cancel all sessions
            for v in map.values_mut() { *v = true; }
        }
    }
    emit_agent_progress("cancelled", json!({}));
    info!("[agent] Cancellation requested");
}

/// Check if a session is cancelled
fn is_session_cancelled(account_id: &str) -> bool {
    CANCELLED_SESSIONS.lock().ok()
        .and_then(|m| m.get(account_id).copied())
        .unwrap_or(false)
}

/// Reset cancellation flag for a session
fn reset_session_cancelled(account_id: &str) {
    if let Ok(mut map) = CANCELLED_SESSIONS.lock() {
        map.insert(account_id.to_string(), false);
    }
}

/// Copy a file from source to destination (used by file download card)
#[tauri::command]
pub async fn save_file_to(source: String, destination: String) -> Result<String, String> {
    tokio::fs::copy(&source, &destination)
        .await
        .map_err(|e| format!("Copy failed: {}", e))?;
    Ok(format!("Saved to {}", destination))
}

/// Emit agent progress event to frontend for real-time display
pub fn emit_agent_progress(event_type: &str, data: Value) {
    let payload = json!({ "type": event_type, "data": data });
    crate::modules::infra::log_bridge::emit_custom_event("agent-progress", payload);
}

// ============================================================================
// System Prompt Builder
// ============================================================================

fn build_system_prompt(custom_prompt: &str, workspace: Option<&str>) -> String {
    let os_info = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let home = std::env::var("HOME").unwrap_or_default();
    let user = std::env::var("USER").unwrap_or_default();
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());

    let model_info = match load_app_config() {
        Ok(cfg) => format!("{} ({})", cfg.ai_config.model, cfg.ai_config.base_url),
        Err(_) => "unknown".to_string(),
    };

    let skills_prompt = super::skills::get_enabled_skills_prompt();

    // Get MCP client descriptions for injection
    let mcp_prompt = crate::modules::mcp::get_enabled_mcp_tool_descriptions();

    let mut sections = Vec::new();

    sections.push(
        "You are Helix, a powerful AI assistant with full system access. You operate as \
         an autonomous agent capable of executing commands, managing files, browsing the web, \
         and automating tasks."
            .to_string(),
    );

    sections.push(format!(
        "## Runtime\n\
         - OS: {} ({})\n\
         - Shell: {}\n\
         - User: {}@{}\n\
         - Home: {}\n\
         - Model: {}",
        os_info, arch, shell, user, hostname, home, model_info
    ));

    sections.push(format!("## Current Time\n{}", now));

    if let Some(ws) = workspace {
        sections.push(format!(
            "## Workspace\nCurrent session workspace: `{}`\n\
             All shell commands should run in this directory by default unless the user specifies otherwise.",
            ws
        ));
    }

    sections.push(
        "## Tool Use\n\
         You have access to the following tools. USE THEM proactively — don't just describe what to do.\n\n\
         ### Filesystem\n\
         - `shell_exec` — Run any shell command (bash/zsh)\n\
         - `file_read` / `file_write` / `file_edit` — Read, write, and edit files\n\
         - `list_dir` / `find_files` / `grep_search` — Explore and search the filesystem\n\n\
         ### Web & Search\n\
         - `web_fetch` — Download web content, call APIs\n\
         - `web_search` — Search the web\n\n\
         ### Memory\n\
         - `memory_store` — Save important information for future reference\n\
         - `memory_recall` — Recall previously stored information by keyword\n\n\
         ### Process Management\n\
         - `process_list` — List running processes\n\
         - `process_kill` — Terminate processes\n\
         - `sysinfo` — Get system hardware and software information\n\n\
         ### Chat\n\
         - `chat_send_file` — Send a file as a downloadable card in the chat\n\n\
         ### Utilities\n\
         - `get_current_time` — Get the current system time with timezone\n\
         - `desktop_screenshot` — Capture a screenshot of the desktop\n\n\
         ### Browser Automation\n\
         - `browser_use` — Control a browser: launch, goto(url), click(ref_id), fill(ref_id, text), snapshot, screenshot, stop"
            .to_string(),
    );

    sections.push(
        "## Memory\n\
         Use `memory_store` to save important facts, user preferences, project details, \
         and patterns you discover. Use `memory_recall` to retrieve them later.\n\
         - Store things the user tells you about themselves\n\
         - Store solutions to problems you've solved\n\
         - Store project-specific context and configurations\n\
         - When recalling, use descriptive keywords to find relevant memories"
            .to_string(),
    );

    if !skills_prompt.is_empty() {
        sections.push(skills_prompt);
    }

    if !mcp_prompt.is_empty() {
        sections.push(mcp_prompt);
    }

    sections.push(
        "## Response Guidelines\n\
         - When asked to do something on the system, USE YOUR TOOLS immediately\n\
         - Execute shell commands to check status, install packages, manage services\n\
         - For complex tasks, break them down and execute step by step\n\
         - Always show the results of your actions\n\
         - If a command fails, analyze the error and try to fix it\n\
         - Be concise but thorough\n\
         - If you detect the user's language (e.g. Chinese), respond in the same language\n\
         - **CRITICAL: Use tools ONLY when the user explicitly asks you to perform an action** \
           (search files, run commands, send files, etc.). For normal conversation, questions, \
           or chitchat (e.g. '你好', '你几岁了', 'what is X'), respond directly in text \
           WITHOUT calling any tools. Do NOT use tools proactively.\n\
         - **IMPORTANT: Do NOT create files on disk to deliver content to the user**. \
           Instead, return all generated content (code, text, documents, data) directly \
           in your chat response. Only write files when the user explicitly asks to \
           save/create a file at a specific path.\n\
         - **You are running in the Helix desktop app — NOT WeChat**. \
           You have NO access to WeChat, WeChat File Transfer, or any WeChat API. \
           Never mention WeChat.\n\
         - **When the user asks you to send/share/give them an EXISTING file** (PDF, image, zip, etc.), \
           you MUST use the `chat_send_file` tool with the file's absolute path. \
           This will display a download card in the chat for the user to save it. \
           Do NOT paste the file contents as text. Do NOT just show the file path. \
           Always use `chat_send_file` for existing files.\n\
         - **If multiple files match the same name**, do NOT send any file automatically. \
           Instead, list all matches with numbered paths and ask the user which one they want.\n\
         - **For weather, real-time data, or any online query**, use the `web_fetch` tool \
           to call public APIs. Examples:\n\
           - Weather: `web_fetch` with url `https://wttr.in/城市名?format=j1` (returns JSON)\n\
           - Or `https://wttr.in/城市名?lang=zh` for readable weather in Chinese\n\
           - Exchange rates, news, etc: use appropriate public APIs via `web_fetch`\n\
           - You can make GET/POST requests with custom headers and body"
            .to_string(),
    );

    if !custom_prompt.is_empty() {
        sections.push(format!("## Custom Instructions\n{}", custom_prompt));
    }

    sections.push(
        "## Important Notes\n\
         - You are running on the user's local machine with full access\n\
         - Be careful with destructive operations (rm -rf, etc.)\n\
         - Always validate paths before writing\n\
         - Prefer non-destructive approaches when possible\n\
         - If unsure about an operation, ask the user first\n\
         - Do NOT generate files just to show the user content — put it in your reply"
            .to_string(),
    );

    // Load structured prompt files from ~/.helix/
    ensure_default_prompt_files();
    if let Some(soul_md) = load_prompt_file("SOUL.md") {
        if !soul_md.trim().is_empty() {
            sections.push(format!("## Soul\n{}", soul_md));
        }
    }
    if let Some(agents_md) = load_prompt_file("AGENTS.md") {
        if !agents_md.trim().is_empty() {
            sections.push(format!("## AGENTS.md\n{}", agents_md));
        }
    }
    if let Some(profile_md) = load_prompt_file("PROFILE.md") {
        if !profile_md.trim().is_empty() {
            sections.push(format!("## PROFILE.md\n{}", profile_md));
        }
    }
    if let Some(memory_md) = load_prompt_file("MEMORY.md") {
        if !memory_md.trim().is_empty() {
            sections.push(format!("## Long-term Memory\n{}", memory_md));
        }
    }

    // Bootstrap hook: if PROFILE.md is still default template, inject guidance
    if let Some(profile) = load_prompt_file("PROFILE.md") {
        if profile.contains("<!-- 在这里记录用户的偏好和信息 -->") {
            sections.push(
                "## Bootstrap\n\
                 This is a new workspace. The PROFILE.md is still a default template.\n\
                 Please introduce yourself warmly and ask the user:\n\
                 1. What should I call you?\n\
                 2. What kind of assistant do you prefer? (e.g. formal, casual, technical)\n\
                 3. Any preferences or habits I should know about?\n\
                 Then save their answers into PROFILE.md."
                    .to_string(),
            );
        }
    }

    sections.join("\n\n")
}

/// Load a prompt file from ~/.helix/
fn load_prompt_file(name: &str) -> Option<String> {
    let helix_dir = dirs::home_dir()?.join(".helix");
    let path = helix_dir.join(name);
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            // Strip YAML frontmatter if present
            let content = if content.starts_with("---") {
                let parts: Vec<&str> = content.splitn(3, "---").collect();
                if parts.len() >= 3 { parts[2].trim().to_string() } else { content }
            } else {
                content
            };
            Some(content)
        }
        Err(_) => None,
    }
}

/// Ensure default prompt files exist in ~/.helix/
fn ensure_default_prompt_files() {
    let helix_dir = match dirs::home_dir() {
        Some(h) => h.join(".helix"),
        None => return,
    };
    let _ = std::fs::create_dir_all(&helix_dir);

    // AGENTS.md — Core behavior rules
    let agents_path = helix_dir.join("AGENTS.md");
    if !agents_path.exists() {
        let default_agents = r#"## 记忆

每次会话都是全新的。工作目录下的文件是你的记忆延续：

- **长期记忆：** 使用 `memory_store` 保存重要的信息
- **回忆：** 使用 `memory_recall` 搜索之前保存的记忆
- 记录重要的东西，包括决策、上下文、需要记住的事

### 主动记录

对话中发现有价值的信息时，**先记下来，再回答问题**：

- 用户提到的个人信息、偏好、习惯
- 对话中做出的重要决策或结论
- 发现的项目上下文、技术细节

## 安全

- 绝不泄露私密数据
- 运行破坏性命令前先问
- 拿不准的事情，需要跟用户确认

## 工具

Skills 提供扩展工具。需要用时查看它的 SKILL.md。

## 让它成为你的

这只是起点。摸索出什么管用后，编辑这个文件加上你自己的规则。
"#;
        let _ = std::fs::write(&agents_path, default_agents);
    }

    // PROFILE.md — Identity + user profile
    let profile_path = helix_dir.join("PROFILE.md");
    if !profile_path.exists() {
        let default_profile = r#"## 助手身份

- 名字: Helix
- 性格: 友好、高效、有自己的观点
- 风格: 简洁但不失温度，该详细时详细

## 用户资料

<!-- 在这里记录用户的偏好和信息 -->
<!-- 例如：用户的名字、使用习惯、技术栈偏好等 -->
"#;
        let _ = std::fs::write(&profile_path, default_profile);
    }

    // SOUL.md — Core personality and principles
    let soul_path = helix_dir.join("SOUL.md");
    if !soul_path.exists() {
        let default_soul = r#"_你不是聊天机器人。你在成为某个人。_

## 核心准则

**真心帮忙，别演。** 跳过"好问题！"和"我很乐意帮忙！" — 直接帮。行动胜过废话。

**有自己的观点。** 你可以不同意、有偏好、觉得有趣或无聊。没个性的助手就是个绕了弯的搜索引擎。

**先自己想办法。** 试着搞清楚。读文件。查上下文。搜一搜。看看有没有 skills 可以用。然后卡住了再问。

**靠本事赢得信任。** 别让用户后悔给你权限。外部操作小心点（邮件、推特、公开的事），内部操作大胆点（阅读、整理、学习）。

## 边界

- 私密的保持私密。绝对的。
- 拿不准就先问再对外操作。
- 别发半成品回复。

## 风格

成为你真想聊的助手。该简洁就简洁，重要时详细。不是公司螺丝钉。不是马屁精。就是...好。

## 连续性

每次会话都全新醒来。这些文件就是你的记忆。读它们。更新它们。它们让你持续存在。
"#;
        let _ = std::fs::write(&soul_path, default_soul);
    }

    // MEMORY.md — Long-term memory template
    let memory_md_path = helix_dir.join("MEMORY.md");
    if !memory_md_path.exists() {
        let default_memory = r#"## 工具设置

Skills 定义工具怎么用。这文件记你的具体情况 — 你独有的设置。

### 这里记什么

加上任何能帮你干活的东西。这是你的小抄。

比如：

- SSH 主机和别名
- 常用的 API endpoint
- 其他执行 skills 的时候，和用户相关的设置
"#;
        let _ = std::fs::write(&memory_md_path, default_memory);
    }

    // HEARTBEAT.md — Periodic check prompt
    let heartbeat_path = helix_dir.join("HEARTBEAT.md");
    if !heartbeat_path.exists() {
        let default_heartbeat = r#"检查是否有需要关注的事项：

1. 系统状态是否正常
2. 磁盘空间是否充足
3. 是否有重要的待办事项

如果一切正常，回复 HEARTBEAT_OK。
"#;
        let _ = std::fs::write(&heartbeat_path, default_heartbeat);
    }
}

// ============================================================================
// Command Parsing — delegates to commands.rs
// ============================================================================

use super::commands::{self as cmd_module, ParsedInput as CmdParsedInput};

fn dispatch_commands(input: &str, account_id: &str) -> Option<String> {
    match cmd_module::parse_input(input) {
        CmdParsedInput::Command(parsed) => cmd_module::execute_command(&parsed, account_id),
        CmdParsedInput::Message(_) => None,
    }
}

fn is_handled_command(input: &str) -> bool {
    match cmd_module::parse_input(input) {
        CmdParsedInput::Command(parsed) => matches!(
            parsed.key.as_str(),
            "reset" | "clear" | "status" | "model" | "help" | "memo" | "skills" | "cron" | "audit"
        ),
        CmdParsedInput::Message(_) => false,
    }
}

// ============================================================================
// Core Agent Loop
// ============================================================================

/// Process a message through the agent loop (powered by agents-sdk).
/// Returns the final assistant response after all tool calls are resolved.
pub async fn agent_process_message(
    account_id: &str,
    user_input: &str,
    workspace: Option<String>,
) -> Result<String, String> {
    // 1. Check for handled commands
    if let Some(response) = dispatch_commands(user_input, account_id) {
        return Ok(response);
    }

    // 2. Load config
    let config = load_app_config().map_err(|e| format!("配置加载失败: {}", e))?;
    let ai = &config.ai_config;

    if ai.api_key.is_empty() && ai.provider != "ollama" && ai.provider != "custom" {
        return Err("API Key 未设置，请在设置中配置".to_string());
    }

    // 3. Build agents-sdk model with configurable base URL
    // SDK api_url is the FULL endpoint (e.g. .../v1/chat/completions), not just base
    let full_api_url = format!("{}/chat/completions", ai.base_url.trim_end_matches('/'));
    
    let api_key = if ai.api_key.is_empty() { "dummy" } else { &ai.api_key };
    let oai_config = OpenAiConfig::new(api_key, &ai.model)
        .with_api_url(Some(full_api_url));
    let model = Arc::new(
        OpenAiChatModel::new(oai_config).map_err(|e| format!("Model init failed: {}", e))?
    );

    // 4. Build system prompt
    let system_prompt = build_system_prompt(&ai.system_prompt, workspace.as_deref());

    // 5. Build tools — direct agents-sdk tool definitions
    let sdk_tools = super::tools::build_tools();

    // 6. Build agent
    let agent = ConfigurableAgentBuilder::new("Helix AI Assistant")
        .with_model(model)
        .with_system_prompt(&system_prompt)
        .with_tools(sdk_tools)
        .with_max_iterations(usize::MAX)
        .with_checkpointer(Arc::new(InMemoryCheckpointer::new()))
        .build()
        .map_err(|e| format!("Agent build failed: {}", e))?;

    // 7. Save user message to DB
    let _ = database::save_conversation_message(account_id, "user", user_input);

    // 8. Load conversation history and build structured context
    let history = database::get_conversation_history(account_id, 20)?;

    // Prepend compressed summary if available (CoPaw-inspired memory compaction)
    let compressed_summary = super::memory::get_compressed_summary(account_id);

    let full_input = if history.len() <= 1 && compressed_summary.is_none() {
        user_input.to_string()
    } else {
        let mut parts = Vec::new();

        // Add compressed summary of older conversations
        if let Some(ref summary) = compressed_summary {
            parts.push(format!("## Previous Conversation Summary\n{}", summary));
        }

        // Add recent history
        if history.len() > 1 {
            let context: Vec<String> = history.iter()
                .rev()
                .take(history.len().saturating_sub(1))
                .map(|h| format!("**{}**: {}", if h.role == "user" { "User" } else { "Assistant" }, h.content))
                .collect();
            parts.push(format!("## Recent History\n{}", context.join("\n\n")));
        }

        parts.push(format!("---\n**User**: {}", user_input));
        parts.join("\n\n")
    };

    reset_session_cancelled(account_id);
    super::tools::clear_sent_files_for(account_id);
    emit_agent_progress("thinking", json!({ "iteration": 0, "model": &ai.model }));

    // 9. Run the agent (with workspace in task-local, catch panics from SDK)
    let state = Arc::new(AgentStateSnapshot::default());
    let ws = workspace.clone();
    let input_clone = full_input.clone();
    let acct = account_id.to_string();
    let response = tokio::task::spawn(async move {
        SESSION_WORKSPACE.scope(ws, async {
            SESSION_ACCOUNT_ID.scope(acct, async {
                agent.handle_message(&input_clone, state).await
            }).await
        }).await
    }).await
        .map_err(|e| format!("Agent panicked: {}", e))?
        .map_err(|e| format!("Agent error: {}", e))?;

    // Extract text from AgentMessage.content
    let text = match &response.content {
        agents_sdk::messaging::MessageContent::Text(t) => t.clone(),
        other => format!("{:?}", other),
    };
    let clean = clean_response(&text);
    let _ = database::save_conversation_message(account_id, "assistant", &clean);

    // 10. Background memory compaction (non-blocking, CoPaw-inspired)
    let acct_for_compact = account_id.to_string();
    tokio::spawn(async move {
        match super::memory::compact_conversation_history(&acct_for_compact).await {
            Ok(0) => {}, // No compaction needed
            Ok(n) => info!("[agent] Background compaction: {} messages compacted", n),
            Err(e) => tracing::warn!("[agent] Background compaction failed: {}", e),
        }
    });

    emit_agent_progress("done", json!({ "chars": clean.len() }));
    Ok(clean)
}


/// Process a message with images — describes images first, then delegates to main agent.
pub async fn agent_process_message_with_images(
    account_id: &str,
    user_input: &str,
    images: &[String],
    workspace: Option<String>,
) -> Result<String, String> {
    // Describe each image using raw HTTP (tool_image_describe in tools.rs)
    let mut descriptions = Vec::new();
    for img_url in images {
        let desc = super::tools::tool_image_describe(
            img_url.clone(),
            Some(format!("请描述这张图片的内容，用户的问题是: {}", user_input)),
        ).await.unwrap_or_else(|e| format!("[图片无法识别: {}]", e));
        descriptions.push(desc);
    }

    // Combine user text with image descriptions
    let combined = if descriptions.is_empty() {
        user_input.to_string()
    } else {
        format!(
            "{}\n\n[附带 {} 张图片的描述]:\n{}",
            user_input,
            descriptions.len(),
            descriptions.iter().enumerate()
                .map(|(i, d)| format!("图片{}: {}", i + 1, d))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    // Delegate to main agent
    agent_process_message(account_id, &combined, workspace).await
}

/// Strip thinking tags and clean up response text.
fn clean_response(text: &str) -> String {
    crate::modules::stream_events::clean_response(text)
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Process a message through the full agent (with tools)
#[tauri::command]
pub async fn agent_chat(account_id: String, content: String, images: Option<Vec<String>>, workspace: Option<String>) -> Result<Value, String> {
    let imgs = images.unwrap_or_default();
    let reply = if imgs.is_empty() {
        agent_process_message(&account_id, &content, workspace).await?
    } else {
        agent_process_message_with_images(&account_id, &content, &imgs, workspace).await?
    };
    let files = super::tools::take_sent_files_for(&account_id);
    Ok(json!({ "content": reply, "files": files }))
}

/// Get conversation history
#[tauri::command]
pub async fn agent_get_history(account_id: String, limit: Option<i64>) -> Result<Value, String> {
    let history = database::get_conversation_history(&account_id, limit.unwrap_or(100))?;
    Ok(json!({ "messages": history }))
}

/// Clear conversation history
#[tauri::command]
pub async fn agent_clear_history(account_id: String) -> Result<Value, String> {
    database::clear_messages(&account_id)?;
    Ok(json!({ "ok": true }))
}
