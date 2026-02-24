//! Multi-Provider Abstraction — Supports OpenAI, Anthropic, Google Gemini, Ollama, and Custom providers.
//!
//! Ported from pi-ai `model-auth.ts` / `models-config.ts`: auto-detects provider from model name,
//! resolves API keys from config/env, and builds provider-specific request payloads.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::info;

// ============================================================================
// Provider Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Google,
    Ollama,
    Custom,
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderKind::OpenAI => write!(f, "openai"),
            ProviderKind::Anthropic => write!(f, "anthropic"),
            ProviderKind::Google => write!(f, "google"),
            ProviderKind::Ollama => write!(f, "ollama"),
            ProviderKind::Custom => write!(f, "custom"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    pub api_key: String,
    pub default_model: Option<String>,
    pub extra_headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedAuth {
    pub api_key: String,
    pub source: String, // "config", "env:OPENAI_API_KEY", etc.
}

// ============================================================================
// Provider Detection
// ============================================================================

/// Model prefix → provider known patterns.
const OPENAI_PREFIXES: &[&str] = &[
    "gpt-", "o1-", "o3-", "o4-", "chatgpt-", "text-", "dall-e",
    "tts-", "whisper", "davinci", "curie", "babbage", "ada",
];
const ANTHROPIC_PREFIXES: &[&str] = &["claude-"];
const GOOGLE_PREFIXES: &[&str] = &["gemini-", "gemma-"];

/// Auto-detect provider from model name.
pub fn detect_provider(model: &str) -> ProviderKind {
    let m = model.to_lowercase();

    for prefix in OPENAI_PREFIXES {
        if m.starts_with(prefix) {
            return ProviderKind::OpenAI;
        }
    }
    for prefix in ANTHROPIC_PREFIXES {
        if m.starts_with(prefix) {
            return ProviderKind::Anthropic;
        }
    }
    for prefix in GOOGLE_PREFIXES {
        if m.starts_with(prefix) {
            return ProviderKind::Google;
        }
    }

    // Assume OpenAI-compatible for unknown models (most common case)
    ProviderKind::OpenAI
}

// ============================================================================
// API Key Resolution
// ============================================================================

/// Environment variable names per provider.
fn env_var_names(kind: &ProviderKind) -> Vec<&'static str> {
    match kind {
        ProviderKind::OpenAI => vec![
            "OPENAI_API_KEY",
            "OPENAI_KEY",
            "AZURE_OPENAI_API_KEY",
        ],
        ProviderKind::Anthropic => vec![
            "ANTHROPIC_API_KEY",
            "CLAUDE_API_KEY",
        ],
        ProviderKind::Google => vec![
            "GEMINI_API_KEY",
            "GOOGLE_AI_KEY",
            "GOOGLE_API_KEY",
        ],
        ProviderKind::Ollama => vec![], // Ollama doesn't need API key
        ProviderKind::Custom => vec![],
    }
}

/// Resolve API key: config > env vars.
pub fn resolve_api_key(
    kind: &ProviderKind,
    config_key: Option<&str>,
) -> ResolvedAuth {
    // 1. Config has priority
    if let Some(key) = config_key {
        if !key.is_empty() {
            return ResolvedAuth {
                api_key: key.to_string(),
                source: "config".to_string(),
            };
        }
    }

    // 2. Environment variables
    for env_name in env_var_names(kind) {
        if let Ok(val) = std::env::var(env_name) {
            if !val.is_empty() {
                return ResolvedAuth {
                    api_key: val,
                    source: format!("env:{}", env_name),
                };
            }
        }
    }

    // 3. Ollama doesn't need a key
    if *kind == ProviderKind::Ollama {
        return ResolvedAuth {
            api_key: String::new(),
            source: "none (ollama)".to_string(),
        };
    }

    ResolvedAuth {
        api_key: String::new(),
        source: "missing".to_string(),
    }
}

// ============================================================================
// Default Base URLs
// ============================================================================

pub fn default_base_url(kind: &ProviderKind) -> &'static str {
    match kind {
        ProviderKind::OpenAI => "https://api.openai.com/v1",
        ProviderKind::Anthropic => "https://api.anthropic.com/v1",
        ProviderKind::Google => "https://generativelanguage.googleapis.com/v1beta",
        ProviderKind::Ollama => "http://127.0.0.1:11434",
        ProviderKind::Custom => "",
    }
}

// ============================================================================
// Provider Config Builder
// ============================================================================

/// Build a full provider config from app config + auto-detection.
pub fn resolve_provider_config(
    model: &str,
    config_base_url: Option<&str>,
    config_api_key: Option<&str>,
    explicit_kind: Option<ProviderKind>,
) -> ProviderConfig {
    let kind = explicit_kind.unwrap_or_else(|| {
        // If config has a custom base_url, check if it looks like a known provider
        if let Some(url) = config_base_url {
            let url_lower = url.to_lowercase();
            if url_lower.contains("anthropic") {
                return ProviderKind::Anthropic;
            }
            if url_lower.contains("googleapis") || url_lower.contains("generativelanguage") {
                return ProviderKind::Google;
            }
            if url_lower.contains("localhost:11434") || url_lower.contains("127.0.0.1:11434") {
                return ProviderKind::Ollama;
            }
        }
        detect_provider(model)
    });

    let auth = resolve_api_key(&kind, config_api_key);
    let base_url = config_base_url
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| default_base_url(&kind))
        .to_string();

    info!(
        "[providers] {} model='{}' base_url='{}' auth={}",
        kind,
        model,
        &base_url[..base_url.len().min(60)],
        auth.source
    );

    ProviderConfig {
        kind,
        base_url,
        api_key: auth.api_key,
        default_model: Some(model.to_string()),
        extra_headers: HashMap::new(),
    }
}

// ============================================================================
// Request Body Builders
// ============================================================================

/// Build OpenAI-compatible chat completion request body.
pub fn build_openai_request(
    model: &str,
    messages: &[Value],
    tools: Option<&[Value]>,
    max_tokens: u32,
    stream: bool,
) -> Value {
    let mut body = json!({
        "model": model,
        "messages": messages,
        "max_tokens": max_tokens,
        "stream": stream,
    });

    if let Some(tools) = tools {
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
    }

    if stream {
        body["stream_options"] = json!({"include_usage": true});
    }

    body
}

/// Build Anthropic messages API request body.
pub fn build_anthropic_request(
    model: &str,
    messages: &[Value],
    system: Option<&str>,
    tools: Option<&[Value]>,
    max_tokens: u32,
    stream: bool,
) -> Value {
    let mut body = json!({
        "model": model,
        "messages": messages,
        "max_tokens": max_tokens,
        "stream": stream,
    });

    if let Some(sys) = system {
        body["system"] = json!(sys);
    }

    if let Some(tools) = tools {
        if !tools.is_empty() {
            // Anthropic uses a different tool format
            let anthropic_tools: Vec<Value> = tools.iter().map(|t| {
                json!({
                    "name": t["function"]["name"],
                    "description": t["function"]["description"],
                    "input_schema": t["function"]["parameters"],
                })
            }).collect();
            body["tools"] = json!(anthropic_tools);
        }
    }

    body
}

/// Build Ollama /api/chat request body.
pub fn build_ollama_request(
    model: &str,
    messages: &[Value],
    tools: Option<&[Value]>,
    stream: bool,
) -> Value {
    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": stream,
    });

    if let Some(tools) = tools {
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
    }

    body
}

// ============================================================================
// Request URL Builders
// ============================================================================

/// Get the chat completion endpoint URL for a provider.
pub fn chat_completion_url(config: &ProviderConfig) -> String {
    let base = config.base_url.trim_end_matches('/');
    match config.kind {
        ProviderKind::OpenAI | ProviderKind::Custom => {
            format!("{}/chat/completions", base)
        }
        ProviderKind::Anthropic => {
            format!("{}/messages", base)
        }
        ProviderKind::Google => {
            let model = config.default_model.as_deref().unwrap_or("gemini-pro");
            format!(
                "{}/models/{}:streamGenerateContent?alt=sse&key={}",
                base, model, config.api_key
            )
        }
        ProviderKind::Ollama => {
            format!("{}/api/chat", base)
        }
    }
}

/// Build authorization headers for a provider.
pub fn auth_headers(config: &ProviderConfig) -> Vec<(String, String)> {
    let mut headers = Vec::new();

    match config.kind {
        ProviderKind::OpenAI | ProviderKind::Custom => {
            if !config.api_key.is_empty() {
                headers.push(("Authorization".into(), format!("Bearer {}", config.api_key)));
            }
            headers.push(("Content-Type".into(), "application/json".into()));
        }
        ProviderKind::Anthropic => {
            headers.push(("x-api-key".into(), config.api_key.clone()));
            headers.push(("anthropic-version".into(), "2023-06-01".into()));
            headers.push(("Content-Type".into(), "application/json".into()));
        }
        ProviderKind::Google => {
            // Google uses query param for auth, but we add content-type
            headers.push(("Content-Type".into(), "application/json".into()));
        }
        ProviderKind::Ollama => {
            headers.push(("Content-Type".into(), "application/json".into()));
        }
    }

    // Add any extra headers from config
    for (k, v) in &config.extra_headers {
        headers.push((k.clone(), v.clone()));
    }

    headers
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn providers_detect(model: String) -> Result<String, String> {
    Ok(detect_provider(&model).to_string())
}

#[tauri::command]
pub async fn providers_resolve(
    model: String,
    base_url: Option<String>,
    api_key: Option<String>,
) -> Result<ProviderConfig, String> {
    Ok(resolve_provider_config(
        &model,
        base_url.as_deref(),
        api_key.as_deref(),
        None,
    ))
}
