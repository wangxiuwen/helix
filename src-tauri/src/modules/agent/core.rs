//! Helix Agent — Powered by agents-sdk.
//!
//! Uses ConfigurableAgentBuilder for the agent loop with custom tools.
//! Multi-modal (images) still uses async-openai directly for now.

use agents_sdk::{
    ConfigurableAgentBuilder, OpenAiConfig, OpenAiChatModel,
    ToolResult, ToolContext, ToolParameterSchema,
    state::AgentStateSnapshot,
    persistence::InMemoryCheckpointer,
};

// async-openai still used for images (multi-modal) path
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

    // 5. Build tools — wrap our existing execute_tool dispatcher
    let sdk_tools = build_sdk_tools().await;

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

    // 8. Load conversation history and build state
    let history = database::get_conversation_history(account_id, 50)?;
    let mut context_msg = String::new();
    for h in &history {
        context_msg.push_str(&format!("[{}]: {}\n", h.role, h.content));
    }
    let full_input = if context_msg.is_empty() {
        user_input.to_string()
    } else {
        format!("{}\n[user]: {}", context_msg, user_input)
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

/// Build agents-sdk Tool wrappers for all our custom tools
async fn build_sdk_tools() -> Vec<Arc<dyn agents_sdk::Tool>> {
    let tool_defs = get_tool_definitions().await;
    let mut sdk_tools: Vec<Arc<dyn agents_sdk::Tool>> = Vec::new();

    for td in tool_defs {
        let func = td.function.clone();
        let name = func.name.clone();
        let desc = func.description.clone().unwrap_or_default();
        let params_json = func.parameters.clone()
            .map(|p| serde_json::to_value(p).unwrap_or(json!({})))
            .unwrap_or(json!({}));

        // Convert our JSON schema to ToolParameterSchema
        let properties = params_json.get("properties").cloned().unwrap_or(json!({}));
        let required: Vec<String> = params_json.get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        // Convert properties from JSON schema to HashMap<String, ToolParameterSchema>
        let properties_map = if let Some(props) = params_json.get("properties").and_then(|p| p.as_object()) {
            let mut map = std::collections::HashMap::new();
            for (key, val) in props {
                let prop_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("string").to_string();
                let prop_desc = val.get("description").and_then(|d| d.as_str()).map(String::from);
                map.insert(key.clone(), ToolParameterSchema {
                    schema_type: prop_type,
                    description: prop_desc,
                    properties: None,
                    required: None,
                    items: None,
                    enum_values: None,
                    default: None,
                    additional: Default::default(),
                });
            }
            Some(map)
        } else {
            None
        };

        let param_schema = ToolParameterSchema {
            schema_type: "object".to_string(),
            description: None,
            properties: properties_map,
            required: Some(required),
            items: None,
            enum_values: None,
            default: None,
            additional: Default::default(),
        };

        let tool_name = name.clone();
        let t = agents_sdk::tool(
            name,
            desc,
            param_schema,
            move |args: Value, ctx: ToolContext| {
                let tn = tool_name.clone();
                async move {
                    info!("[agent-sdk] Tool call: {}", tn);
                    emit_agent_progress("tool_call", json!({ "name": &tn }));

                    let result = match execute_tool(&tn, &args, None).await {
                        Ok(r) => r,
                        Err(e) => format!("Error: {}", e),
                    };

                    info!("[agent-sdk] Tool result: {} chars", result.len());
                    emit_agent_progress("tool_result", json!({ "name": &tn, "chars": result.len() }));

                    Ok(ToolResult::text(&ctx, result))
                }
            },
        );
        sdk_tools.push(t);
    }

    sdk_tools
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

    // Agent loop — same as agent_process_message
    let tool_defs = get_tool_definitions().await;
    let max_iterations = 20;

    for iteration in 0..max_iterations {
        info!("[agent] Iteration {}, msgs={}", iteration, messages.len());
        let request = CreateChatCompletionRequestArgs::default()
            .model(&ai.model)
            .messages(messages.clone())
            .tools(tool_defs.clone())
            .build()
            .map_err(|e| format!("Build request failed: {}", e))?;

        let chat = client.chat();
        let api_future = chat.create(request);
        let cancel_future = async {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if AGENT_CANCELLED.load(Ordering::SeqCst) { break; }
            }
        };
        let response = tokio::select! {
            result = api_future => result.map_err(|e| format!("AI call failed: {}", e))?,
            _ = cancel_future => { return Err("⏹ 已停止".to_string()); }
        };

        let choice = response.choices.first().ok_or("No choices")?;
        let finish_reason = choice.finish_reason.as_ref()
            .map(|r| format!("{:?}", r))
            .unwrap_or_default()
            .to_lowercase();
        let has_tools = choice.message.tool_calls
            .as_ref()
            .map_or(false, |tcs| !tcs.is_empty());

        if finish_reason.contains("stop") || !has_tools {
            let content = choice.message.content.clone().unwrap_or_default();
            let clean = clean_response(&content);
            let _ = database::save_conversation_message(account_id, "assistant", &clean);
            return Ok(clean);
        }

        let tcs = choice.message.tool_calls.as_ref().unwrap();
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
    }

    // Force final text response
    let final_request = CreateChatCompletionRequestArgs::default()
        .model(&ai.model)
        .messages(messages.clone())
        .build()
        .map_err(|e| format!("Build request failed: {}", e))?;
    let chat = client.chat();
    let api_future = chat.create(final_request);
    let cancel_future = async {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if AGENT_CANCELLED.load(Ordering::SeqCst) { break; }
        }
    };
    let final_response = tokio::select! {
        result = api_future => result.map_err(|e| format!("AI call failed: {}", e))?,
        _ = cancel_future => { return Err("⏹ 已停止".to_string()); }
    };
    let content = final_response.choices.first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_else(|| "任务已完成".to_string());
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
