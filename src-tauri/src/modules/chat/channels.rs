//! Channel System — Abstract channel/plugin architecture for Helix.
//!
//! Unified channel registry, message routing, and session management.
//! Supports: DingTalk, Telegram, Discord, QQ, iMessage, Feishu.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;

// ============================================================================
// Channel Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ChannelId {
    #[serde(rename = "dingtalk")]
    DingTalk,
    #[serde(rename = "telegram")]
    Telegram,
    #[serde(rename = "discord")]
    Discord,
    #[serde(rename = "qq")]
    QQ,
    #[serde(rename = "imessage")]
    IMessage,
    #[serde(rename = "feishu")]
    Feishu,
    #[serde(rename = "wecom")]
    WeCom,
    #[serde(rename = "custom")]
    Custom(String),
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelId::DingTalk => write!(f, "dingtalk"),
            ChannelId::Telegram => write!(f, "telegram"),
            ChannelId::Discord => write!(f, "discord"),
            ChannelId::QQ => write!(f, "qq"),
            ChannelId::IMessage => write!(f, "imessage"),
            ChannelId::Feishu => write!(f, "feishu"),
            ChannelId::WeCom => write!(f, "wecom"),
            ChannelId::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMeta {
    pub id: ChannelId,
    pub label: String,
    pub description: String,
    pub icon: String,
    pub supports_auto_reply: bool,
    pub supports_media: bool,
    pub connected: bool,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub channel: ChannelId,
    pub session_key: String,
    pub sender: String,
    pub sender_id: String,
    pub content: String,
    pub msg_type: String,
    pub media_url: Option<String>,
    pub timestamp: i64,
    pub raw: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub channel: ChannelId,
    pub session_key: String,
    pub content: String,
    pub reply_to: Option<String>,
}

// ============================================================================
// Channel Registry
// ============================================================================

pub fn list_channels() -> Vec<ChannelMeta> {
    vec![
        ChannelMeta {
            id: ChannelId::DingTalk,
            label: "钉钉".into(),
            description: "DingTalk webhook / Robot 推送通知".into(),
            icon: "🔔".into(),
            supports_auto_reply: false,
            supports_media: false,
            connected: false,
            protocol: "webhook".into(),
        },
        ChannelMeta {
            id: ChannelId::Telegram,
            label: "Telegram".into(),
            description: "Telegram Bot API 双向消息".into(),
            icon: "✈️".into(),
            supports_auto_reply: true,
            supports_media: true,
            connected: false,
            protocol: "api".into(),
        },
        ChannelMeta {
            id: ChannelId::Discord,
            label: "Discord".into(),
            description: "Discord Bot 双向消息".into(),
            icon: "🎮".into(),
            supports_auto_reply: true,
            supports_media: true,
            connected: false,
            protocol: "api".into(),
        },
        ChannelMeta {
            id: ChannelId::QQ,
            label: "QQ".into(),
            description: "QQ 机器人 (OpenShamrock/Napcat)".into(),
            icon: "🐧".into(),
            supports_auto_reply: true,
            supports_media: true,
            connected: false,
            protocol: "onebot".into(),
        },
        ChannelMeta {
            id: ChannelId::IMessage,
            label: "iMessage".into(),
            description: "Apple iMessage (macOS only)".into(),
            icon: "💬".into(),
            supports_auto_reply: true,
            supports_media: true,
            connected: false,
            protocol: "applescript".into(),
        },
        ChannelMeta {
            id: ChannelId::Feishu,
            label: "飞书".into(),
            description: "飞书 / Lark Bot API".into(),
            icon: "🪶".into(),
            supports_auto_reply: true,
            supports_media: true,
            connected: false,
            protocol: "api".into(),
        },
        ChannelMeta {
            id: ChannelId::WeCom,
            label: "企业微信".into(),
            description: "企业微信群机器人 Webhook".into(),
            icon: "💼".into(),
            supports_auto_reply: false,
            supports_media: false,
            connected: false,
            protocol: "webhook".into(),
        },
    ]
}

pub fn resolve_channel_id(raw: &str) -> Option<ChannelId> {
    let normalized = raw.trim().to_lowercase();
    match normalized.as_str() {
        "dingtalk" | "ding" | "钉钉" => Some(ChannelId::DingTalk),
        "telegram" | "tg" | "电报" => Some(ChannelId::Telegram),
        "discord" | "dc" => Some(ChannelId::Discord),
        "qq" => Some(ChannelId::QQ),
        "imessage" | "imsg" | "apple" => Some(ChannelId::IMessage),
        "feishu" | "lark" | "飞书" => Some(ChannelId::Feishu),
        "wecom" | "wechat_work" | "企业微信" | "企微" => Some(ChannelId::WeCom),
        _ => None,
    }
}

pub fn get_channel_meta(id: &ChannelId) -> Option<ChannelMeta> {
    list_channels().into_iter().find(|c| c.id == *id)
}

// ============================================================================
// Message Router
// ============================================================================

pub async fn route_inbound_message(msg: &InboundMessage) -> Result<String, String> {
    info!(
        "[{}] Inbound from {}: '{}'",
        msg.channel,
        msg.sender,
        &msg.content[..msg.content.len().min(50)]
    );

    let reply = crate::modules::agent::agent_process_message(&msg.session_key, &msg.content, None).await?;

    info!(
        "[{}] Reply: '{}'",
        msg.channel,
        &reply[..reply.len().min(50)]
    );

    Ok(reply)
}

pub async fn dispatch_outbound_message(msg: &OutboundMessage) -> Result<(), String> {
    match &msg.channel {
        ChannelId::DingTalk => {
            let config = crate::modules::config::load_app_config().map_err(|e| e.to_string())?;
            if let Some(ref notif) = config.notifications {
                if let Some(ref url) = notif.dingtalk_webhook {
                    crate::modules::notifications::send_dingtalk(url, "Helix", &msg.content).await
                } else {
                    Err("DingTalk webhook not configured".into())
                }
            } else {
                Err("Notifications not configured".into())
            }
        }
        ChannelId::Telegram => Err("Telegram channel not yet implemented. Configure TELEGRAM_BOT_TOKEN in Environments.".into()),
        ChannelId::Discord => {
            // Discord Bot via HTTP API
            let token = std::env::var("DISCORD_BOT_TOKEN")
                .map_err(|_| "DISCORD_BOT_TOKEN not set. Add it in Settings → Environments.")?;
            let channel_id = &msg.session_key;
            let url = format!("https://discord.com/api/v10/channels/{}/messages", channel_id);

            let client = reqwest::Client::new();
            let resp = client.post(&url)
                .header("Authorization", format!("Bot {}", token))
                .json(&serde_json::json!({ "content": msg.content }))
                .send()
                .await
                .map_err(|e| format!("Discord API error: {}", e))?;

            if resp.status().is_success() {
                Ok(())
            } else {
                let err = resp.text().await.unwrap_or_default();
                Err(format!("Discord API error: {}", &err[..err.len().min(300)]))
            }
        }
        ChannelId::QQ => {
            // QQ via OneBot v11 HTTP API (compatible with NapCat/OpenShamrock)
            let api_url = std::env::var("QQ_ONEBOT_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
            let url = format!("{}/send_msg", api_url.trim_end_matches('/'));

            let body = serde_json::json!({
                "message_type": "private",
                "user_id": msg.session_key,
                "message": msg.content,
            });

            let client = reqwest::Client::new();
            let resp = client.post(&url)
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("QQ OneBot error: {}", e))?;

            if resp.status().is_success() {
                Ok(())
            } else {
                let err = resp.text().await.unwrap_or_default();
                Err(format!("QQ OneBot error: {}", &err[..err.len().min(300)]))
            }
        }
        ChannelId::IMessage => {
            // iMessage via AppleScript (macOS only)
            #[cfg(target_os = "macos")]
            {
                let escaped = msg.content.replace("\"", "\\\"").replace("\\", "\\\\");
                let script = format!(
                    "tell application \"Messages\"\n  set targetBuddy to \"{}\"\n  set targetService to id of 1st account whose service type = iMessage\n  set theBuddy to participant targetBuddy of account id targetService\n  send \"{}\" to theBuddy\nend tell",
                    msg.session_key, escaped
                );

                let output = tokio::process::Command::new("osascript")
                    .arg("-e")
                    .arg(&script)
                    .output()
                    .await
                    .map_err(|e| format!("AppleScript error: {}", e))?;

                if output.status.success() {
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!("iMessage send failed: {}", stderr))
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err("iMessage is only available on macOS".into())
            }
        }
        ChannelId::Feishu => {
            // Feishu/Lark webhook
            let webhook_url = std::env::var("FEISHU_WEBHOOK_URL")
                .map_err(|_| "FEISHU_WEBHOOK_URL not set. Add it in Settings → Environments.")?;

            let body = serde_json::json!({
                "msg_type": "text",
                "content": { "text": msg.content }
            });

            let client = reqwest::Client::new();
            let resp = client.post(&webhook_url)
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Feishu webhook error: {}", e))?;

            if resp.status().is_success() {
                Ok(())
            } else {
                let err = resp.text().await.unwrap_or_default();
                Err(format!("Feishu webhook error: {}", &err[..err.len().min(300)]))
            }
        }
        ChannelId::WeCom => {
            // 企业微信群机器人 Webhook
            let webhook_url = std::env::var("WECOM_WEBHOOK_URL")
                .map_err(|_| "WECOM_WEBHOOK_URL not set. Add it in Settings → Environments.")?;

            let body = serde_json::json!({
                "msgtype": "text",
                "text": { "content": msg.content }
            });

            let client = reqwest::Client::new();
            let resp = client.post(&webhook_url)
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("WeCom webhook error: {}", e))?;

            if resp.status().is_success() {
                Ok(())
            } else {
                let err = resp.text().await.unwrap_or_default();
                Err(format!("WeCom webhook error: {}", &err[..err.len().min(300)]))
            }
        }
        ChannelId::Custom(name) => Err(format!("Custom channel '{}' not implemented", name)),
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn channels_list() -> Result<Vec<ChannelMeta>, String> {
    Ok(list_channels())
}

#[tauri::command]
pub async fn channels_send(channel: String, session_key: String, content: String) -> Result<(), String> {
    let channel_id = resolve_channel_id(&channel).ok_or_else(|| format!("Unknown channel: {}", channel))?;
    dispatch_outbound_message(&OutboundMessage {
        channel: channel_id,
        session_key,
        content,
        reply_to: None,
    })
    .await
}

#[tauri::command]
pub async fn channels_resolve(raw: String) -> Result<Option<String>, String> {
    Ok(resolve_channel_id(&raw).map(|id| id.to_string()))
}
