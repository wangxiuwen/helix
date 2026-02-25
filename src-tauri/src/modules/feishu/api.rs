//! Feishu REST API — token management and message sending.
//!
//! All HTTP calls to the Feishu Open Platform API go through this module.

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;

use super::{FEISHU_STATE, FEISHU_API_BASE, TokenCache};

// ============================================================================
// Response types
// ============================================================================

#[derive(Debug, Deserialize)]
struct TokenResponse {
    code: i64,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    tenant_access_token: String,
    #[serde(default)]
    expire: u64, // seconds until expiry
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    code: i64,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    data: Option<Value>,
}

// ============================================================================
// Token Management
// ============================================================================

/// Get a valid tenant_access_token, refreshing if necessary.
pub async fn get_token() -> Result<String, String> {
    // Check cached token first
    {
        let state = FEISHU_STATE.lock().await;
        if state.token.is_valid() {
            return Ok(state.token.tenant_access_token.clone());
        }
    }

    // Refresh token
    refresh_token().await
}

/// Force refresh the tenant_access_token.
async fn refresh_token() -> Result<String, String> {
    let (app_id, app_secret) = {
        let state = FEISHU_STATE.lock().await;
        if state.config.app_id.is_empty() || state.config.app_secret.is_empty() {
            return Err("Feishu appId/appSecret not configured".to_string());
        }
        (state.config.app_id.clone(), state.config.app_secret.clone())
    };

    let client = reqwest::Client::new();
    let url = format!("{}/open-apis/auth/v3/tenant_access_token/internal", FEISHU_API_BASE);

    let resp = client.post(&url)
        .header(CONTENT_TYPE, "application/json; charset=utf-8")
        .json(&json!({
            "app_id": app_id,
            "app_secret": app_secret,
        }))
        .send()
        .await
        .map_err(|e| format!("Feishu token request failed: {}", e))?;

    let token_resp: TokenResponse = resp.json()
        .await
        .map_err(|e| format!("Feishu token response parse failed: {}", e))?;

    if token_resp.code != 0 {
        return Err(format!("Feishu token error: code={}, msg={}", token_resp.code, token_resp.msg));
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let token = token_resp.tenant_access_token.clone();
    let expires_at = now + token_resp.expire;

    // Update cache
    {
        let mut state = FEISHU_STATE.lock().await;
        state.token = TokenCache {
            tenant_access_token: token.clone(),
            expires_at,
        };
    }

    info!("[Feishu] Token refreshed, expires in {}s", token_resp.expire);
    Ok(token)
}

// ============================================================================
// Send Message
// ============================================================================

/// Send a text message to a chat.
pub async fn send_text_message(chat_id: &str, text: &str) -> Result<String, String> {
    let token = get_token().await?;
    let client = reqwest::Client::new();
    let url = format!(
        "{}/open-apis/im/v1/messages?receive_id_type=chat_id",
        FEISHU_API_BASE
    );

    let content = json!({ "text": text }).to_string();

    let resp = client.post(&url)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json; charset=utf-8")
        .json(&json!({
            "receive_id": chat_id,
            "msg_type": "text",
            "content": content,
        }))
        .send()
        .await
        .map_err(|e| format!("Feishu send message failed: {}", e))?;

    let api_resp: ApiResponse = resp.json()
        .await
        .map_err(|e| format!("Feishu send response parse failed: {}", e))?;

    if api_resp.code != 0 {
        return Err(format!("Feishu send error: code={}, msg={}", api_resp.code, api_resp.msg));
    }

    let message_id = api_resp.data
        .as_ref()
        .and_then(|d| d.get("message_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    info!("[Feishu] Sent text message to {}, msg_id={}", chat_id, message_id);
    Ok(message_id)
}

/// Reply to a specific message.
pub async fn reply_message(message_id: &str, text: &str) -> Result<String, String> {
    let token = get_token().await?;
    let client = reqwest::Client::new();
    let url = format!(
        "{}/open-apis/im/v1/messages/{}/reply",
        FEISHU_API_BASE, message_id
    );

    let content = json!({ "text": text }).to_string();

    let resp = client.post(&url)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json; charset=utf-8")
        .json(&json!({
            "msg_type": "text",
            "content": content,
        }))
        .send()
        .await
        .map_err(|e| format!("Feishu reply failed: {}", e))?;

    let api_resp: ApiResponse = resp.json()
        .await
        .map_err(|e| format!("Feishu reply response parse failed: {}", e))?;

    if api_resp.code != 0 {
        return Err(format!("Feishu reply error: code={}, msg={}", api_resp.code, api_resp.msg));
    }

    let reply_msg_id = api_resp.data
        .as_ref()
        .and_then(|d| d.get("message_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    info!("[Feishu] Replied to {}, new_msg_id={}", message_id, reply_msg_id);
    Ok(reply_msg_id)
}

/// Add an emoji reaction to a message.
pub async fn add_reaction(message_id: &str, emoji_type: &str) -> Result<String, String> {
    let token = get_token().await?;
    let client = reqwest::Client::new();
    let url = format!(
        "{}/open-apis/im/v1/messages/{}/reactions",
        FEISHU_API_BASE, message_id
    );

    let resp = client.post(&url)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json; charset=utf-8")
        .json(&json!({
            "reaction_type": {
                "emoji_type": emoji_type
            }
        }))
        .send()
        .await
        .map_err(|e| format!("Feishu add reaction failed: {}", e))?;

    let api_resp: ApiResponse = resp.json()
        .await
        .map_err(|e| format!("Feishu reaction response parse failed: {}", e))?;

    if api_resp.code != 0 {
        return Err(format!("Feishu reaction error: code={}, msg={}", api_resp.code, api_resp.msg));
    }

    // Return reaction_id for deletion later
    let reaction_id = api_resp.data
        .as_ref()
        .and_then(|d| d.get("reaction_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    info!("[Feishu] Added {} reaction to {}", emoji_type, message_id);
    Ok(reaction_id)
}

/// Delete an emoji reaction from a message.
pub async fn delete_reaction(message_id: &str, reaction_id: &str) -> Result<(), String> {
    let token = get_token().await?;
    let client = reqwest::Client::new();
    let url = format!(
        "{}/open-apis/im/v1/messages/{}/reactions/{}",
        FEISHU_API_BASE, message_id, reaction_id
    );

    let resp = client.delete(&url)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("Feishu delete reaction failed: {}", e))?;

    let api_resp: ApiResponse = resp.json()
        .await
        .map_err(|e| format!("Feishu delete reaction response parse failed: {}", e))?;

    if api_resp.code != 0 {
        return Err(format!("Feishu delete reaction error: code={}, msg={}", api_resp.code, api_resp.msg));
    }

    Ok(())
}

/// Send a markdown card message to a chat.
pub async fn send_card_message(chat_id: &str, markdown_text: &str) -> Result<String, String> {
    let token = get_token().await?;
    let client = reqwest::Client::new();
    let url = format!(
        "{}/open-apis/im/v1/messages?receive_id_type=chat_id",
        FEISHU_API_BASE
    );

    let card = json!({
        "config": { "wide_screen_mode": true },
        "elements": [
            { "tag": "markdown", "content": markdown_text }
        ]
    });

    let content = card.to_string();

    let resp = client.post(&url)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json; charset=utf-8")
        .json(&json!({
            "receive_id": chat_id,
            "msg_type": "interactive",
            "content": content,
        }))
        .send()
        .await
        .map_err(|e| format!("Feishu send card failed: {}", e))?;

    let api_resp: ApiResponse = resp.json()
        .await
        .map_err(|e| format!("Feishu send card response parse failed: {}", e))?;

    if api_resp.code != 0 {
        return Err(format!("Feishu send card error: code={}, msg={}", api_resp.code, api_resp.msg));
    }

    let message_id = api_resp.data
        .as_ref()
        .and_then(|d| d.get("message_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    info!("[Feishu] Sent card message to {}, msg_id={}", chat_id, message_id);
    Ok(message_id)
}

/// Send a text message to a user by open_id.
pub async fn send_text_to_user(open_id: &str, text: &str) -> Result<String, String> {
    let token = get_token().await?;
    let client = reqwest::Client::new();
    let url = format!(
        "{}/open-apis/im/v1/messages?receive_id_type=open_id",
        FEISHU_API_BASE
    );

    let content = json!({ "text": text }).to_string();

    let resp = client.post(&url)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json; charset=utf-8")
        .json(&json!({
            "receive_id": open_id,
            "msg_type": "text",
            "content": content,
        }))
        .send()
        .await
        .map_err(|e| format!("Feishu send to user failed: {}", e))?;

    let api_resp: ApiResponse = resp.json()
        .await
        .map_err(|e| format!("Feishu send to user response parse failed: {}", e))?;

    if api_resp.code != 0 {
        return Err(format!("Feishu send to user error: code={}, msg={}", api_resp.code, api_resp.msg));
    }

    let message_id = api_resp.data
        .as_ref()
        .and_then(|d| d.get("message_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    info!("[Feishu] Sent text to user {}, msg_id={}", open_id, message_id);
    Ok(message_id)
}

// ============================================================================
// WebSocket Endpoint Discovery
// ============================================================================

#[derive(Debug, Deserialize)]
pub(crate) struct WsEndpointResponse {
    pub code: i64,
    #[serde(default)]
    pub msg: String,
    #[serde(default)]
    pub data: Option<WsEndpointData>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WsEndpointData {
    #[serde(rename = "URL")]
    pub url: String,
    #[serde(rename = "ClientConfig", default)]
    pub client_config: Option<Value>,
}

/// Get the WebSocket endpoint URL for event subscription.
pub(crate) async fn get_ws_endpoint() -> Result<WsEndpointData, String> {
    let (app_id, app_secret) = {
        let state = FEISHU_STATE.lock().await;
        if state.config.app_id.is_empty() || state.config.app_secret.is_empty() {
            return Err("Feishu appId/appSecret not configured".to_string());
        }
        (state.config.app_id.clone(), state.config.app_secret.clone())
    };

    let client = reqwest::Client::new();
    let url = format!("{}/callback/ws/endpoint", FEISHU_API_BASE);

    let resp = client.post(&url)
        .header(CONTENT_TYPE, "application/json")
        .header("locale", "zh")
        .json(&json!({
            "AppID": app_id,
            "AppSecret": app_secret
        }))
        .send()
        .await
        .map_err(|e| format!("Feishu WS endpoint request failed: {}", e))?;

    // Read raw body first to debug parse failures
    let body = resp.text()
        .await
        .map_err(|e| format!("Feishu WS endpoint read body failed: {}", e))?;

    let ws_resp: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| {
            tracing::error!("[Feishu] WS endpoint raw response: {}", &body[..body.len().min(500)]);
            format!("Feishu WS endpoint parse failed: {} | body={}", e, &body[..body.len().min(200)])
        })?;

    let code = ws_resp.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    let msg = ws_resp.get("msg").and_then(|v| v.as_str()).unwrap_or("unknown");

    if code != 0 {
        return Err(format!("飞书 WS 端点错误: code={}, msg={}", code, msg));
    }

    let data = ws_resp.get("data").ok_or("Feishu WS endpoint: no data")?;
    let ws_data: WsEndpointData = serde_json::from_value(data.clone())
        .map_err(|e| format!("Feishu WS endpoint data parse failed: {}", e))?;

    Ok(ws_data)
}
