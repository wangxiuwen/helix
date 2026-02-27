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
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};

use crate::modules::config::load_app_config;
use crate::modules::database;

/// Global cancellation flag — set to true to stop the running agent loop
static AGENT_CANCELLED: AtomicBool = AtomicBool::new(false);

/// Cancel the currently running agent
#[tauri::command]
pub fn agent_cancel() {
    AGENT_CANCELLED.store(true, Ordering::SeqCst);
    emit_agent_progress("cancelled", json!({}));
    info!("[agent] Cancellation requested");
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

fn build_system_prompt(custom_prompt: &str) -> String {
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
         - `chat_send_file` — Send a file as a downloadable card in the chat"
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

    sections.join("\n\n")
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
) -> Result<String, String> {
    // 1. Check for handled commands
    if let Some(response) = dispatch_commands(user_input, account_id) {
        return Ok(response);
    }

    // 2. Load config
    let config = load_app_config().map_err(|e| format!("配置加载失败: {}", e))?;
    let ai = &config.ai_config;

    if ai.api_key.is_empty() {
        return Err("API Key 未设置，请在设置中配置".to_string());
    }

    // 3. Build agents-sdk model with configurable base URL
    // SDK api_url is the FULL endpoint (e.g. .../v1/chat/completions), not just base
    let full_api_url = format!("{}/chat/completions", ai.base_url.trim_end_matches('/'));
    let oai_config = OpenAiConfig::new(&ai.api_key, &ai.model)
        .with_api_url(Some(full_api_url));
    let model = Arc::new(
        OpenAiChatModel::new(oai_config).map_err(|e| format!("Model init failed: {}", e))?
    );

    // 4. Build system prompt
    let system_prompt = build_system_prompt(&ai.system_prompt);

    // 5. Build tools — direct agents-sdk tool definitions
    let sdk_tools = super::tools::build_tools();

    // 6. Build agent
    let agent = ConfigurableAgentBuilder::new("Helix AI Assistant")
        .with_model(model)
        .with_system_prompt(&system_prompt)
        .with_tools(sdk_tools)
        .with_checkpointer(Arc::new(InMemoryCheckpointer::new()))
        .build()
        .map_err(|e| format!("Agent build failed: {}", e))?;

    // 7. Save user message to DB
    let _ = database::save_conversation_message(account_id, "user", user_input);

    // 8. Load conversation history and build structured context
    let history = database::get_conversation_history(account_id, 20)?;
    let full_input = if history.len() <= 1 {
        user_input.to_string()
    } else {
        let context: Vec<String> = history.iter()
            .rev()
            .take(history.len().saturating_sub(1))
            .map(|h| format!("**{}**: {}", if h.role == "user" { "User" } else { "Assistant" }, h.content))
            .collect();
        format!("## Conversation History\n{}\n\n---\n**User**: {}", context.join("\n\n"), user_input)
    };

    AGENT_CANCELLED.store(false, Ordering::SeqCst);
    super::tools::clear_sent_files();
    emit_agent_progress("thinking", json!({ "iteration": 0, "model": &ai.model }));

    // 9. Run the agent
    let state = Arc::new(AgentStateSnapshot::default());
    let response = agent.handle_message(&full_input, state).await
        .map_err(|e| format!("Agent error: {}", e))?;

    // Extract text from AgentMessage.content
    let text = match &response.content {
        agents_sdk::messaging::MessageContent::Text(t) => t.clone(),
        other => format!("{:?}", other),
    };
    let clean = clean_response(&text);
    let _ = database::save_conversation_message(account_id, "assistant", &clean);
    emit_agent_progress("done", json!({ "chars": clean.len() }));
    Ok(clean)
}


/// Process a message with images — describes images first, then delegates to main agent.
pub async fn agent_process_message_with_images(
    account_id: &str,
    user_input: &str,
    images: &[String],
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
    agent_process_message(account_id, &combined).await
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
pub async fn agent_chat(account_id: String, content: String, images: Option<Vec<String>>) -> Result<Value, String> {
    let imgs = images.unwrap_or_default();
    let reply = if imgs.is_empty() {
        agent_process_message(&account_id, &content).await?
    } else {
        agent_process_message_with_images(&account_id, &content, &imgs).await?
    };
    let files = super::tools::take_sent_files();
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
