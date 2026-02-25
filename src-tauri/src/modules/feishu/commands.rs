//! Tauri commands for Feishu channel — frontend IPC.

use serde_json::{json, Value};
use tracing::info;

use super::{FEISHU_STATE, FeishuConfig, save_config_to_disk};
use super::api;
use super::gateway;

// ============================================================================
// Config commands
// ============================================================================

/// Get Feishu configuration
#[tauri::command]
pub async fn feishu_get_config() -> Result<Value, String> {
    let state = FEISHU_STATE.lock().await;
    Ok(json!({
        "app_id": state.config.app_id,
        "app_secret": state.config.app_secret,
        "bot_name": state.config.bot_name,
        "enabled": state.config.enabled,
        "connected": state.gateway_connected,
        "has_secret": !state.config.app_secret.is_empty(),
    }))
}

/// Save Feishu configuration
#[tauri::command]
pub async fn feishu_save_config(
    app_id: String,
    app_secret: String,
    bot_name: String,
    enabled: bool,
) -> Result<Value, String> {
    let mut state = FEISHU_STATE.lock().await;
    // If app_secret is empty, keep the existing one (frontend doesn't send the secret back)
    let actual_secret = if app_secret.is_empty() {
        state.config.app_secret.clone()
    } else {
        app_secret
    };
    state.config = FeishuConfig {
        app_id: app_id.clone(),
        app_secret: actual_secret,
        bot_name,
        enabled,
    };
    save_config_to_disk(&state.config);

    info!("[Feishu] Config saved: app_id={}, enabled={}", app_id, enabled);
    Ok(json!({ "ok": true }))
}

// ============================================================================
// Gateway commands
// ============================================================================

/// Connect to Feishu WebSocket gateway
#[tauri::command]
pub async fn feishu_connect() -> Result<Value, String> {
    gateway::start_gateway().await?;
    Ok(json!({ "ok": true, "status": "connected" }))
}

/// Disconnect from Feishu WebSocket gateway
#[tauri::command]
pub async fn feishu_disconnect() -> Result<Value, String> {
    gateway::stop_gateway().await?;
    Ok(json!({ "ok": true, "status": "disconnected" }))
}

/// Get Feishu connection status
#[tauri::command]
pub async fn feishu_get_status() -> Result<Value, String> {
    let state = FEISHU_STATE.lock().await;
    Ok(json!({
        "connected": state.gateway_connected,
        "configured": !state.config.app_id.is_empty() && !state.config.app_secret.is_empty(),
        "enabled": state.config.enabled,
        "app_id": state.config.app_id,
        "bot_name": state.config.bot_name,
        "token_valid": state.token.is_valid(),
    }))
}

// ============================================================================
// Message commands
// ============================================================================

/// Send a text message to a Feishu chat
#[tauri::command]
pub async fn feishu_send_message(chat_id: String, content: String) -> Result<Value, String> {
    let msg_id = api::send_text_message(&chat_id, &content).await?;

    // Save to DB
    let account_id = format!("feishu:{}", chat_id);
    let _ = crate::modules::database::save_message(&account_id, &content, true, 1, false);

    Ok(json!({
        "ok": true,
        "message_id": msg_id,
    }))
}

/// Send a text message to a Feishu user by open_id
#[tauri::command]
pub async fn feishu_send_to_user(open_id: String, content: String) -> Result<Value, String> {
    let msg_id = api::send_text_to_user(&open_id, &content).await?;
    Ok(json!({
        "ok": true,
        "message_id": msg_id,
    }))
}

/// Test Feishu connection by getting a token
#[tauri::command]
pub async fn feishu_test_connection() -> Result<Value, String> {
    let token = api::get_token().await?;
    Ok(json!({
        "ok": true,
        "token_prefix": &token[..token.len().min(10)],
    }))
}
