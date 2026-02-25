//! Channel System — Abstract channel/plugin architecture for Helix.
//!
//! Treats WeChat File Helper as a "channel" (like OpenClaw's Telegram/WhatsApp/DingTalk plugins),
//! with a unified channel registry, message routing, and session management.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;

// ============================================================================
// Channel Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ChannelId {
    #[serde(rename = "wechat_filehelper")]
    WeChatFileHelper,
    #[serde(rename = "feishu")]
    Feishu,
    #[serde(rename = "dingtalk")]
    DingTalk,
    #[serde(rename = "telegram")]
    Telegram,
    #[serde(rename = "custom")]
    Custom(String),
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelId::WeChatFileHelper => write!(f, "wechat_filehelper"),
            ChannelId::Feishu => write!(f, "feishu"),
            ChannelId::DingTalk => write!(f, "dingtalk"),
            ChannelId::Telegram => write!(f, "telegram"),
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
    /// Whether this channel supports auto-reply
    pub supports_auto_reply: bool,
    /// Whether this channel supports media messages
    pub supports_media: bool,
    /// Whether the channel is currently connected/active
    pub connected: bool,
    /// Protocol type: "webhook", "websocket", "web_protocol", "api"
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub channel: ChannelId,
    pub session_key: String,
    pub sender: String,
    pub sender_id: String,
    pub content: String,
    pub msg_type: String, // "text", "image", "audio", "video", "file"
    pub media_url: Option<String>,
    pub timestamp: i64,
    /// Raw provider-specific data
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

/// Get all registered channel metadata.
pub fn list_channels() -> Vec<ChannelMeta> {
    vec![
        ChannelMeta {
            id: ChannelId::WeChatFileHelper,
            label: "微信文件传输助手".into(),
            description: "通过 WeChat Web 协议连接微信文件传输助手，支持全双工消息和 AI 自动回复".into(),
            icon: "💬".into(),
            supports_auto_reply: true,
            supports_media: false, // text only via web protocol
            connected: false,
            protocol: "web_protocol".into(),
        },
        ChannelMeta {
            id: ChannelId::Feishu,
            label: "飞书".into(),
            description: "通过 Webhook 发送飞书通知消息".into(),
            icon: "🐦".into(),
            supports_auto_reply: false,
            supports_media: false,
            connected: false,
            protocol: "webhook".into(),
        },
        ChannelMeta {
            id: ChannelId::DingTalk,
            label: "钉钉".into(),
            description: "通过 Webhook 发送钉钉通知消息".into(),
            icon: "🔔".into(),
            supports_auto_reply: false,
            supports_media: false,
            connected: false,
            protocol: "webhook".into(),
        },
        ChannelMeta {
            id: ChannelId::Telegram,
            label: "Telegram".into(),
            description: "Telegram Bot API (预留)".into(),
            icon: "✈️".into(),
            supports_auto_reply: true,
            supports_media: true,
            connected: false,
            protocol: "api".into(),
        },
    ]
}

/// Resolve a channel ID from a string (supports aliases).
pub fn resolve_channel_id(raw: &str) -> Option<ChannelId> {
    let normalized = raw.trim().to_lowercase();
    match normalized.as_str() {
        "wechat" | "wechat_filehelper" | "filehelper" | "微信" | "文件传输助手" => {
            Some(ChannelId::WeChatFileHelper)
        }
        "feishu" | "lark" | "飞书" => Some(ChannelId::Feishu),
        "dingtalk" | "ding" | "钉钉" => Some(ChannelId::DingTalk),
        "telegram" | "tg" | "电报" => Some(ChannelId::Telegram),
        _ => None,
    }
}

/// Get metadata for a specific channel.
pub fn get_channel_meta(id: &ChannelId) -> Option<ChannelMeta> {
    list_channels().into_iter().find(|c| c.id == *id)
}

// ============================================================================
// Message Router
// ============================================================================

/// Route an inbound message to the agent and get the reply.
/// This is the unified entry point for all channels.
pub async fn route_inbound_message(msg: &InboundMessage) -> Result<String, String> {
    info!(
        "[{}] Inbound from {}: '{}'",
        msg.channel,
        msg.sender,
        &msg.content[..msg.content.len().min(50)]
    );

    // Use the full agent loop to process
    let reply = crate::modules::agent::agent_process_message(&msg.session_key, &msg.content).await?;

    info!(
        "[{}] Reply: '{}'",
        msg.channel,
        &reply[..reply.len().min(50)]
    );

    Ok(reply)
}

/// Dispatch an outbound message to the appropriate channel.
pub async fn dispatch_outbound_message(msg: &OutboundMessage) -> Result<(), String> {
    match &msg.channel {
        ChannelId::WeChatFileHelper => {
            crate::modules::filehelper::filehelper_send_msg(msg.session_key.clone(), msg.content.clone())
                .await
                .map(|_| ())
        }
        ChannelId::Feishu => {
            let config = crate::modules::config::load_app_config().map_err(|e| e.to_string())?;
            if let Some(ref notif) = config.notifications {
                if let Some(ref url) = notif.feishu_webhook {
                    crate::modules::notifications::send_feishu(url, "Helix", &msg.content).await
                } else {
                    Err("Feishu webhook not configured".into())
                }
            } else {
                Err("Notifications not configured".into())
            }
        }
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
        ChannelId::Telegram => Err("Telegram channel not yet implemented".into()),
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
