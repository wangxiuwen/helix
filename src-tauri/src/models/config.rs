use serde::{Deserialize, Serialize};
use crate::modules::cloudflared::CloudflaredConfig;

/// AI model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiModelConfig {
    /// Provider name: "ark", "openai", "custom"
    pub provider: String,
    /// API base URL (OpenAI-compatible)
    pub base_url: String,
    /// API key
    pub api_key: String,
    /// Model identifier
    pub model: String,
    /// Max output tokens
    pub max_tokens: u32,
    /// System prompt for AI replies
    pub system_prompt: String,
    /// Enable auto-reply for WeChat File Helper messages
    pub auto_reply: bool,
}

impl Default for AiModelConfig {
    fn default() -> Self {
        Self {
            provider: "ark".to_string(),
            base_url: "https://ark.cn-beijing.volces.com/api/coding/v3".to_string(),
            api_key: String::new(),
            model: "ark-code-latest".to_string(),
            max_tokens: 4096,
            system_prompt: "你是一个智能助手，通过微信文件传输助手与用户对话。请用简洁、友好的中文回复。".to_string(),
            auto_reply: false,
        }
    }
}

/// Notification webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotificationsConfig {
    /// Feishu bot webhook URL
    #[serde(default)]
    pub feishu_webhook: Option<String>,
    /// DingTalk robot webhook URL
    #[serde(default)]
    pub dingtalk_webhook: Option<String>,
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub language: String,
    pub theme: String,
    pub auto_refresh: bool,
    pub refresh_interval: i32,  // minutes
    pub auto_sync: bool,
    pub sync_interval: i32,  // minutes
    pub default_export_path: Option<String>,
    #[serde(default)]
    pub auto_launch: bool,  // Launch on startup
    #[serde(default)]
    pub hidden_menu_items: Vec<String>, // Hidden menu item path list
    #[serde(default)]
    pub cloudflared: CloudflaredConfig, // Cloudflared configuration
    #[serde(default)]
    pub ai_config: AiModelConfig, // AI model configuration
    #[serde(default)]
    pub notifications: Option<NotificationsConfig>, // Notification webhook config
    #[serde(default)]
    pub search_api_key: Option<String>, // Brave Search API key
}

impl AppConfig {
    pub fn new() -> Self {
        Self {
            language: "zh".to_string(),
            theme: "system".to_string(),
            auto_refresh: true,
            refresh_interval: 15,
            auto_sync: false,
            sync_interval: 5,
            default_export_path: None,
            auto_launch: false,
            hidden_menu_items: Vec::new(),
            cloudflared: CloudflaredConfig::default(),
            ai_config: AiModelConfig::default(),
            notifications: None,
            search_api_key: None,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::new()
    }
}
