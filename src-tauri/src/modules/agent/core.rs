//! Helix Agent — Powered by async-openai SDK.
//!
//! Clean agent loop: system prompt → user message → AI call → tool execution → repeat.

use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs,
        ChatCompletionRequestUserMessageContent,
        ChatCompletionRequestMessageContentPartText,
        ChatCompletionRequestMessageContentPartImage,
        ImageUrl,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use serde_json::{json, Value};
use tracing::info;
use tauri::Emitter;
use std::sync::atomic::{AtomicBool, Ordering};

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
fn emit_agent_progress(event_type: &str, data: Value) {
    let payload = json!({ "type": event_type, "data": data });
    crate::modules::infra::log_bridge::emit_custom_event("agent-progress", payload);
}

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
         - `sysinfo` — Get system hardware and software information"
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
           Always use `chat_send_file` for existing files."
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

    // 6. Single-pass agent: one AI call → execute tools → one final AI call
    AGENT_CANCELLED.store(false, Ordering::SeqCst);
    super::tools::clear_sent_files();

    // Check cancellation
    if AGENT_CANCELLED.load(Ordering::SeqCst) {
        return Err("⏹ 已停止".to_string());
    }

    // First AI call
    emit_agent_progress("thinking", json!({ "iteration": 0, "model": &ai.model }));
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

    // If no tool calls, return text directly
    let tool_calls = &choice.message.tool_calls;
    let has_tools = tool_calls.as_ref().map_or(false, |tcs| !tcs.is_empty());

    if !has_tools {
        let content = choice.message.content.clone().unwrap_or_default();
        let clean = clean_response(&content);
        let _ = database::save_conversation_message(account_id, "assistant", &clean);
        emit_agent_progress("done", json!({ "chars": clean.len() }));
        return Ok(clean);
    }

    // Execute tool calls
    let tcs = tool_calls.as_ref().unwrap();
    let assistant_msg = ChatCompletionRequestAssistantMessageArgs::default()
        .tool_calls(tcs.clone())
        .build()
        .map_err(|e| e.to_string())?;
    messages.push(assistant_msg.into());

    for tc in tcs {
        if AGENT_CANCELLED.load(Ordering::SeqCst) {
            return Err("⏹ 已停止".to_string());
        }
        let tool_name = &tc.function.name;
        info!("[agent] Tool call: {} (id={})", tool_name, tc.id);
        emit_agent_progress("tool_call", json!({ "name": tool_name, "args": &tc.function.arguments }));

        let args: Value = serde_json::from_str(&tc.function.arguments)
            .unwrap_or(json!({}));

        let result = match execute_tool(tool_name, &args, Some(account_id)).await {
            Ok(r) => r,
            Err(e) => format!("Error: {}", e),
        };

        info!("[agent] Tool result: {} chars", result.len());
        emit_agent_progress("tool_result", json!({ "name": tool_name, "chars": result.len(), "preview": &result[..result.len().min(200)] }));

        let tool_msg = ChatCompletionRequestToolMessageArgs::default()
            .content(result)
            .tool_call_id(tc.id.clone())
            .build()
            .map_err(|e| e.to_string())?;
        messages.push(tool_msg.into());
    }

    // Final AI call — get text response after tools
    if AGENT_CANCELLED.load(Ordering::SeqCst) {
        return Err("⏹ 已停止".to_string());
    }
    emit_agent_progress("thinking", json!({ "iteration": 1, "model": &ai.model }));
    let final_request = CreateChatCompletionRequestArgs::default()
        .model(&ai.model)
        .messages(messages.clone())
        .tools(tool_defs)
        .build()
        .map_err(|e| format!("Build request failed: {}", e))?;

    let final_response = client
        .chat()
        .create(final_request)
        .await
        .map_err(|e| format!("AI call failed: {}", e))?;

    let final_choice = final_response
        .choices
        .first()
        .ok_or("No choices in final response")?;

    let content = final_choice.message.content.clone().unwrap_or_default();
    let clean = clean_response(&content);
    let _ = database::save_conversation_message(account_id, "assistant", &clean);
    emit_agent_progress("done", json!({ "chars": clean.len() }));
    Ok(clean)
}

/// Process a message with images through the agent loop (multi-modal).
pub async fn agent_process_message_with_images(
    account_id: &str,
    user_input: &str,
    images: &[String],
) -> Result<String, String> {
    // Check for handled commands first
    if let Some(response) = dispatch_commands(user_input, account_id) {
        return Ok(response);
    }

    let config = load_app_config().map_err(|e| format!("配置加载失败: {}", e))?;
    let ai = &config.ai_config;
    if ai.api_key.is_empty() {
        return Err("API Key 未设置".to_string());
    }

    let openai_config = OpenAIConfig::new()
        .with_api_base(&ai.base_url)
        .with_api_key(&ai.api_key);
    let client = Client::with_config(openai_config);

    let system_prompt = build_system_prompt(&ai.system_prompt);
    let mut messages: Vec<ChatCompletionRequestMessage> = Vec::new();

    messages.push(
        ChatCompletionRequestSystemMessageArgs::default()
            .content(system_prompt)
            .build()
            .map_err(|e| e.to_string())?
            .into(),
    );

    // Load conversation history (text only)
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

    // Build multi-modal user message: text + images
    use async_openai::types::ChatCompletionRequestUserMessageContentPart;
    let mut parts: Vec<ChatCompletionRequestUserMessageContentPart> = Vec::new();
    parts.push(ChatCompletionRequestUserMessageContentPart::Text(
        ChatCompletionRequestMessageContentPartText { text: user_input.to_string() },
    ));
    for img_url in images {
        parts.push(ChatCompletionRequestUserMessageContentPart::ImageUrl(
            ChatCompletionRequestMessageContentPartImage {
                image_url: ImageUrl { url: img_url.clone(), detail: None },
            },
        ));
    }

    let user_msg = ChatCompletionRequestUserMessageArgs::default()
        .content(ChatCompletionRequestUserMessageContent::Array(parts))
        .build()
        .map_err(|e| e.to_string())?;
    messages.push(user_msg.into());
    let _ = database::save_conversation_message(account_id, "user", user_input);

    info!("[agent] Multi-modal message with {} images", images.len());

    // Single-pass agent (same as agent_process_message)
    let tool_defs = get_tool_definitions().await;

    // First AI call
    let request = CreateChatCompletionRequestArgs::default()
        .model(&ai.model)
        .messages(messages.clone())
        .tools(tool_defs.clone())
        .build()
        .map_err(|e| format!("Build request failed: {}", e))?;

    let response = client.chat().create(request).await
        .map_err(|e| format!("AI call failed: {}", e))?;

    let choice = response.choices.first().ok_or("No choices")?;
    let tool_calls = &choice.message.tool_calls;
    let has_tools = tool_calls.as_ref().map_or(false, |tcs| !tcs.is_empty());

    if !has_tools {
        let content = choice.message.content.clone().unwrap_or_default();
        let clean = clean_response(&content);
        let _ = database::save_conversation_message(account_id, "assistant", &clean);
        return Ok(clean);
    }

    // Execute tool calls
    let tcs = tool_calls.as_ref().unwrap();
    let assistant_msg = ChatCompletionRequestAssistantMessageArgs::default()
        .tool_calls(tcs.clone())
        .build()
        .map_err(|e| e.to_string())?;
    messages.push(assistant_msg.into());

    for tc in tcs {
        let tool_name = &tc.function.name;
        info!("[agent] Tool call: {} (id={})", tool_name, tc.id);
        let args: Value = serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));
        let result = match execute_tool(tool_name, &args, Some(account_id)).await {
            Ok(r) => r,
            Err(e) => format!("Error: {}", e),
        };
        let tool_msg = ChatCompletionRequestToolMessageArgs::default()
            .content(result)
            .tool_call_id(tc.id.clone())
            .build()
            .map_err(|e| e.to_string())?;
        messages.push(tool_msg.into());
    }

    // Final AI call
    let final_request = CreateChatCompletionRequestArgs::default()
        .model(&ai.model)
        .messages(messages.clone())
        .tools(tool_defs)
        .build()
        .map_err(|e| format!("Build request failed: {}", e))?;

    let final_response = client.chat().create(final_request).await
        .map_err(|e| format!("AI call failed: {}", e))?;

    let final_choice = final_response.choices.first().ok_or("No choices in final response")?;
    let content = final_choice.message.content.clone().unwrap_or_default();
    let clean = clean_response(&content);
    let _ = database::save_conversation_message(account_id, "assistant", &clean);
    Ok(clean)
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
