//! Notification integrations — Feishu & DingTalk webhook senders.
//!
//! Provides a unified `send_notification(channel, title, body)` API
//! used by cron jobs, hooks, and other modules.

use reqwest::Client;
use serde_json::json;
use tracing::info;

use super::config;

// ============================================================================
// Public API
// ============================================================================

/// Send a notification to the specified channel.
/// `channel` — "feishu" or "dingtalk"
pub async fn send_notification(channel: &str, title: &str, body: &str) -> Result<(), String> {
    let webhook_url = get_webhook_url(channel)?;

    match channel {
        "feishu" => send_feishu(&webhook_url, title, body).await,
        "dingtalk" => send_dingtalk(&webhook_url, title, body).await,
        _ => Err(format!("Unknown notification channel: {}", channel)),
    }
}

/// Test a webhook URL by sending a test message.
pub async fn test_webhook(channel: &str, webhook_url: &str) -> Result<String, String> {
    let title = "🔔 Helix 通知测试";
    let body = "这是一条来自 Helix 的测试通知，如果您看到此消息说明 Webhook 配置正确！";

    match channel {
        "feishu" => send_feishu(webhook_url, title, body).await?,
        "dingtalk" => send_dingtalk(webhook_url, title, body).await?,
        _ => return Err(format!("Unknown channel: {}", channel)),
    }

    Ok("通知发送成功".to_string())
}

// ============================================================================
// Feishu Webhook
// ============================================================================

pub async fn send_feishu(webhook_url: &str, title: &str, body: &str) -> Result<(), String> {
    let client = Client::new();

    let payload = json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": {
                    "tag": "plain_text",
                    "content": title
                },
                "template": "blue"
            },
            "elements": [
                {
                    "tag": "markdown",
                    "content": body
                }
            ]
        }
    });

    let resp = client
        .post(webhook_url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Feishu webhook request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Feishu webhook returned {}: {}", status, text));
    }

    info!("Feishu notification sent: {}", title);
    Ok(())
}

// ============================================================================
// DingTalk Webhook
// ============================================================================

pub async fn send_dingtalk(webhook_url: &str, title: &str, body: &str) -> Result<(), String> {
    let client = Client::new();

    let payload = json!({
        "msgtype": "markdown",
        "markdown": {
            "title": title,
            "text": format!("## {}\n\n{}", title, body)
        }
    });

    let resp = client
        .post(webhook_url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("DingTalk webhook request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("DingTalk webhook returned {}: {}", status, text));
    }

    info!("DingTalk notification sent: {}", title);
    Ok(())
}

// ============================================================================
// Config Helpers
// ============================================================================

fn get_webhook_url(channel: &str) -> Result<String, String> {
    let cfg = config::load_app_config()?;

    // Look in config.notifications.feishu_webhook / dingtalk_webhook
    let url = match channel {
        "feishu" => cfg.notifications.as_ref()
            .and_then(|n| n.feishu_webhook.as_ref())
            .cloned(),
        "dingtalk" => cfg.notifications.as_ref()
            .and_then(|n| n.dingtalk_webhook.as_ref())
            .cloned(),
        _ => None,
    };

    url.filter(|u| !u.is_empty())
        .ok_or_else(|| format!("No webhook URL configured for channel '{}'", channel))
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn notification_test_send(channel: String, webhook_url: String) -> Result<String, String> {
    test_webhook(&channel, &webhook_url).await
}
