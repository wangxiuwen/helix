//! AI Chat module — OpenAI-compatible API calls for auto-reply.
//!
//! Reads config from helix_config.json and provides chat completions.

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, error};

use crate::models::config::AiModelConfig;
use crate::modules::config::{load_app_config, save_app_config};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMessage {
    pub role: String,   // "system", "user", "assistant"
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<AiUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ============================================================================
// Core AI call
// ============================================================================

/// Call an OpenAI-compatible chat completions endpoint.
pub async fn chat_complete(
    config: &AiModelConfig,
    messages: Vec<AiMessage>,
) -> Result<AiChatResponse, String> {
    if config.api_key.is_empty() {
        return Err("API Key 未设置，请在设置中配置".to_string());
    }

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", config.api_key))
            .map_err(|e| format!("Invalid API key: {}", e))?,
    );

    let body = json!({
        "model": config.model,
        "messages": messages,
        "max_tokens": config.max_tokens,
        "stream": false,
    });

    info!(
        "AI request: provider={}, model={}, url={}, messages={}",
        config.provider, config.model, config.base_url, messages.len()
    );

    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let resp = client
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("AI API 请求失败: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        error!("AI API error: status={}, body={}", status, &err_body[..err_body.len().min(500)]);
        return Err(format!("AI API 返回错误 ({}): {}", status, &err_body[..err_body.len().min(200)]));
    }

    let data: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 AI 响应失败: {}", e))?;

    let content = data["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let model = data["model"]
        .as_str()
        .unwrap_or(&config.model)
        .to_string();

    let usage = if !data["usage"].is_null() {
        Some(AiUsage {
            prompt_tokens: data["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: data["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: data["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
        })
    } else {
        None
    };

    info!(
        "AI response: model={}, content_len={}, tokens={:?}",
        model,
        content.len(),
        usage.as_ref().map(|u| u.total_tokens)
    );

    // Record usage in unified tracking
    if let Some(ref u) = usage {
        let _ = super::usage::record_usage(
            "auto_reply",
            &model,
            &config.provider,
            u.prompt_tokens,
            u.completion_tokens,
            "auto_reply",
        );
    }

    Ok(AiChatResponse {
        content,
        model,
        usage,
    })
}

/// Process a WeChat message and generate an AI reply.
/// Auto-reply enable/disable is checked by the caller (filehelper per-account).
pub async fn process_wechat_message(content: &str) -> Result<String, String> {
    let config = load_app_config()
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let ai = &config.ai_config;

    if ai.api_key.is_empty() {
        return Err("API Key 未设置".to_string());
    }

    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: ai.system_prompt.clone(),
        },
        AiMessage {
            role: "user".to_string(),
            content: content.to_string(),
        },
    ];

    let resp = chat_complete(ai, messages).await?;
    Ok(resp.content)
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Send a message to the AI and get a reply (manual test)
#[tauri::command]
pub async fn ai_chat_send(content: String) -> Result<Value, String> {
    let config = load_app_config()
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let ai = &config.ai_config;

    let messages = vec![
        AiMessage {
            role: "system".to_string(),
            content: ai.system_prompt.clone(),
        },
        AiMessage {
            role: "user".to_string(),
            content,
        },
    ];

    let resp = chat_complete(ai, messages).await?;

    Ok(json!({
        "content": resp.content,
        "model": resp.model,
        "usage": resp.usage,
    }))
}

/// Get current AI config
#[tauri::command]
pub async fn ai_get_config() -> Result<Value, String> {
    let config = load_app_config()
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let ai = &config.ai_config;

    Ok(json!({
        "provider": ai.provider,
        "base_url": ai.base_url,
        "api_key": if ai.api_key.is_empty() { "".to_string() } else { format!("{}****", &ai.api_key[..ai.api_key.len().min(8)]) },
        "api_key_set": !ai.api_key.is_empty(),
        "model": ai.model,
        "max_tokens": ai.max_tokens,
        "system_prompt": ai.system_prompt,
        "auto_reply": ai.auto_reply,
    }))
}

/// Update AI config
#[tauri::command]
pub async fn ai_set_config(
    provider: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    max_tokens: Option<u32>,
    system_prompt: Option<String>,
    auto_reply: Option<bool>,
) -> Result<Value, String> {
    let mut config = load_app_config()
        .map_err(|e| format!("读取配置失败: {}", e))?;

    if let Some(v) = provider { config.ai_config.provider = v; }
    if let Some(v) = base_url { config.ai_config.base_url = v; }
    if let Some(v) = api_key { config.ai_config.api_key = v; }
    if let Some(v) = model { config.ai_config.model = v; }
    if let Some(v) = max_tokens { config.ai_config.max_tokens = v; }
    if let Some(v) = system_prompt { config.ai_config.system_prompt = v; }
    if let Some(v) = auto_reply { config.ai_config.auto_reply = v; }

    save_app_config(&config)
        .map_err(|e| format!("保存配置失败: {}", e))?;

    info!("AI config updated: provider={}, model={}", config.ai_config.provider, config.ai_config.model);

    Ok(json!({ "ok": true }))
}

/// Test AI connection
#[tauri::command]
pub async fn ai_test_connection() -> Result<Value, String> {
    let config = load_app_config()
        .map_err(|e| format!("读取配置失败: {}", e))?;
    let ai = &config.ai_config;

    if ai.api_key.is_empty() {
        return Err("请先设置 API Key".to_string());
    }

    let messages = vec![
        AiMessage {
            role: "user".to_string(),
            content: "你好，请简短回复一个字以确认连接正常。".to_string(),
        },
    ];

    let resp = chat_complete(ai, messages).await?;

    Ok(json!({
        "ok": true,
        "reply": resp.content,
        "model": resp.model,
        "usage": resp.usage,
    }))
}
