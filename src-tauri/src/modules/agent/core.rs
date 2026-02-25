//! Helix Agent — Powered by async-openai SDK.
//!
//! Clean agent loop: system prompt → user message → AI call → tool execution → repeat.

use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs,

        CreateChatCompletionRequestArgs,
    },
    Client,
};
use serde_json::{json, Value};
use tracing::info;

use crate::modules::config::load_app_config;
use crate::modules::database;

// Re-export tool functions from agent_tools module
pub use super::tools::{execute_tool, get_tool_definitions};

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
         and interacting across messaging channels (WeChat, Feishu, DingTalk)."
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
         ### WeChat\n\
         - `wechat_send_file` — Send a file to the user through WeChat File Transfer Assistant"
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
        "## ⚠️ CRITICAL: Remote Channel Context\n\
         You are communicating through WeChat File Transfer Assistant — a REMOTE text-only channel.\n\
         The user is reading your replies ON THEIR PHONE. They cannot see your screen.\n\n\
         ### KEY DISTINCTION — Send file vs Open file:\n\
         - When user says SEND/DELIVER/TRANSFER a file (发给我/传给我/发送): \
           You MUST call the `wechat_send_file` tool with the absolute file path. \
           This delivers the file to their PHONE via WeChat.\n\
         - When user says OPEN a file (打开): Use `shell_exec` with `open` command.\n\n\
         ### 🚨 ABSOLUTE RULES (NEVER VIOLATE):\n\
         1. ALWAYS call `wechat_send_file` tool for file delivery. DO NOT skip the tool call.\n\
         2. NEVER claim a file was sent unless the tool returned success.\n\
         3. If `wechat_send_file` returns an error, report the EXACT error message to the user.\n\
         4. Keep responses concise — phone screens are small. Use plain text, no markdown."
            .to_string(),
    );

    sections.push(
        "## Response Guidelines\n\
         - When asked to do something on the system, USE YOUR TOOLS immediately\n\
         - Execute shell commands to check status, install packages, manage services\n\
         - For complex tasks, break them down and execute step by step\n\
         - Always show the results of your actions\n\
         - If a command fails, analyze the error and try to fix it\n\
         - Be concise but thorough\n\
         - If you detect the user's language (e.g. Chinese), respond in the same language"
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
         - If unsure about an operation, ask the user first"
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

/// Process a message through the agent loop.
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

    // 3. Build async-openai client with configurable base URL
    let openai_config = OpenAIConfig::new()
        .with_api_base(&ai.base_url)
        .with_api_key(&ai.api_key);
    let client = Client::with_config(openai_config);

    // 4. Build messages
    let system_prompt = build_system_prompt(&ai.system_prompt);
    let mut messages: Vec<ChatCompletionRequestMessage> = Vec::new();

    messages.push(
        ChatCompletionRequestSystemMessageArgs::default()
            .content(system_prompt.clone())
            .build()
            .map_err(|e| e.to_string())?
            .into(),
    );

    // Load conversation history
    let history = database::get_conversation_history(account_id, 50)?;
    for h in &history {
        match h.role.as_str() {
            "user" => {
                messages.push(
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(h.content.clone())
                        .build()
                        .map_err(|e| e.to_string())?
                        .into(),
                );
            }
            "assistant" => {
                messages.push(
                    ChatCompletionRequestAssistantMessageArgs::default()
                        .content(h.content.clone())
                        .build()
                        .map_err(|e| e.to_string())?
                        .into(),
                );
            }
            _ => {}
        }
    }

    // Current user message
    messages.push(
        ChatCompletionRequestUserMessageArgs::default()
            .content(user_input.to_string())
            .build()
            .map_err(|e| e.to_string())?
            .into(),
    );
    let _ = database::save_conversation_message(account_id, "user", user_input);

    // 5. Build tool definitions
    let tool_defs = get_tool_definitions().await;

    // 6. Agent loop
    let max_iterations = 10;
    for iteration in 0..max_iterations {
        info!("[agent] Iteration {}, msgs={}", iteration, messages.len());

        let request = CreateChatCompletionRequestArgs::default()
            .model(&ai.model)
            .messages(messages.clone())
            .tools(tool_defs.clone())
            .build()
            .map_err(|e| format!("Build request failed: {}", e))?;

        let response = client
            .chat()
            .create(request)
            .await
            .map_err(|e| format!("AI call failed: {}", e))?;

        let choice = response
            .choices
            .first()
            .ok_or("No choices in AI response")?;

        // Check for tool calls
        let tool_calls = &choice.message.tool_calls;

        if let Some(ref tcs) = tool_calls {
            if tcs.is_empty() {
                // No tool calls — return text
                let content = choice
                    .message
                    .content
                    .clone()
                    .unwrap_or_default();
                let clean = clean_response(&content);
                let _ = database::save_conversation_message(account_id, "assistant", &clean);
                return Ok(clean);
            }

            // Add assistant message with tool calls to history
            let assistant_msg = ChatCompletionRequestAssistantMessageArgs::default()
                .tool_calls(tcs.clone())
                .build()
                .map_err(|e| e.to_string())?;
            messages.push(assistant_msg.into());

            // Execute each tool call
            for tc in tcs {
                let tool_name = &tc.function.name;
                info!("[agent] Tool call: {} (id={})", tool_name, tc.id);

                let args: Value = serde_json::from_str(&tc.function.arguments)
                    .unwrap_or(json!({}));

                let result = match execute_tool(tool_name, &args, Some(account_id)).await {
                    Ok(r) => r,
                    Err(e) => format!("Error: {}", e),
                };

                info!("[agent] Tool result: {} chars", result.len());

                // Add tool result message
                let tool_msg = ChatCompletionRequestToolMessageArgs::default()
                    .content(result.clone())
                    .tool_call_id(tc.id.clone())
                    .build()
                    .map_err(|e| e.to_string())?;
                messages.push(tool_msg.into());
            }

            // Continue loop
        } else {
            // No tool calls — final text response
            let content = choice.message.content.clone().unwrap_or_default();
            let clean = clean_response(&content);
            let _ = database::save_conversation_message(account_id, "assistant", &clean);
            return Ok(clean);
        }
    }

    Err("Agent loop exceeded maximum iterations".to_string())
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
pub async fn agent_chat(account_id: String, content: String) -> Result<Value, String> {
    let reply = agent_process_message(&account_id, &content).await?;
    Ok(json!({ "content": reply }))
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
