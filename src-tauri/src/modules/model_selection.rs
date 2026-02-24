//! Model Selection & Alias Resolution — Ported from OpenClaw `model-selection.ts`.
//!
//! Resolves model aliases (e.g. "sonnet" → "claude-sonnet-4-5"), normalizes
//! provider IDs, enforces allowlists, and selects default models per agent.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

use super::config::load_app_config;

// ============================================================================
// Core Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRef {
    pub provider: String,
    pub model: String,
}

impl std::fmt::Display for ModelRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.provider, self.model)
    }
}

// ============================================================================
// Built-in Model Aliases
// ============================================================================

/// Built-in model aliases — quick shorthand for popular models.
fn builtin_aliases() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        // Anthropic
        ("sonnet", "anthropic", "claude-sonnet-4-5-20250514"),
        ("claude", "anthropic", "claude-sonnet-4-5-20250514"),
        ("haiku", "anthropic", "claude-haiku-3-5-20250620"),
        ("opus", "anthropic", "claude-opus-4-6-20260115"),
        // OpenAI
        ("gpt4o", "openai", "gpt-4o"),
        ("gpt4", "openai", "gpt-4o"),
        ("4o", "openai", "gpt-4o"),
        ("4o-mini", "openai", "gpt-4o-mini"),
        ("o1", "openai", "o1"),
        ("o1-mini", "openai", "o1-mini"),
        ("o3", "openai", "o3"),
        ("o3-mini", "openai", "o3-mini"),
        ("o4-mini", "openai", "o4-mini"),
        ("chatgpt", "openai", "chatgpt-4o-latest"),
        // Google
        ("gemini", "google", "gemini-2.5-flash"),
        ("gemini-pro", "google", "gemini-2.5-pro"),
        ("flash", "google", "gemini-2.5-flash"),
        // Ollama local
        ("llama", "ollama", "llama3.3"),
        ("qwen", "ollama", "qwen2.5"),
        ("deepseek", "ollama", "deepseek-r1"),
        ("mistral", "ollama", "mistral"),
        ("phi", "ollama", "phi3"),
    ]
}

// ============================================================================
// Provider Normalization
// ============================================================================

/// Normalize provider ID to canonical form.
pub fn normalize_provider_id(provider: &str) -> String {
    let p = provider.trim().to_lowercase();
    match p.as_str() {
        "openai" | "open_ai" | "open-ai" => "openai".to_string(),
        "anthropic" | "claude" => "anthropic".to_string(),
        "google" | "google-ai" | "gemini" | "vertex" | "vertex-ai" => "google".to_string(),
        "ollama" | "local" => "ollama".to_string(),
        "azure" | "azure-openai" | "azure_openai" => "azure-openai".to_string(),
        "together" | "together-ai" => "together".to_string(),
        "groq" => "groq".to_string(),
        "deepseek" => "deepseek".to_string(),
        "moonshot" | "kimi" => "moonshot".to_string(),
        "zhipu" | "glm" | "chatglm" => "zhipu".to_string(),
        "qwen" | "tongyi" | "dashscope" => "qwen".to_string(),
        "yi" | "lingyiwanwu" => "yi".to_string(),
        "minimax" => "minimax".to_string(),
        "baichuan" => "baichuan".to_string(),
        other => other.to_string(),
    }
}

// ============================================================================
// Alias Resolution
// ============================================================================

/// Resolve a model string that might be an alias, a "provider/model" pair,
/// or a raw model ID.
pub fn resolve_model_ref(raw: &str, config_aliases: &HashMap<String, ModelRef>) -> ModelRef {
    let input = raw.trim();
    let lower = input.to_lowercase();

    // 1. Check config-defined aliases first (user overrides)
    if let Some(resolved) = config_aliases.get(&lower) {
        info!("[model] alias '{}' → {}", input, resolved);
        return resolved.clone();
    }

    // 2. Check built-in aliases
    for (alias, provider, model) in builtin_aliases() {
        if lower == alias {
            let r = ModelRef {
                provider: provider.to_string(),
                model: model.to_string(),
            };
            info!("[model] built-in alias '{}' → {}", input, r);
            return r;
        }
    }

    // 3. Check "provider/model" format
    if let Some(slash_idx) = input.find('/') {
        let provider = normalize_provider_id(&input[..slash_idx]);
        let model = input[slash_idx + 1..].to_string();
        return ModelRef { provider, model };
    }

    // 4. Auto-detect provider from model name
    let provider = super::providers::detect_provider(input);
    ModelRef {
        provider: provider.to_string(),
        model: input.to_string(),
    }
}

/// Parse model aliases from config. Expected format in config:
/// ```json
/// { "model_aliases": { "fast": "openai/gpt-4o-mini", "smart": "anthropic/claude-sonnet-4-5" } }
/// ```
pub fn parse_config_aliases(raw: &HashMap<String, String>) -> HashMap<String, ModelRef> {
    let mut aliases = HashMap::new();
    for (alias, target) in raw {
        let lower = alias.trim().to_lowercase();
        if let Some(slash) = target.find('/') {
            aliases.insert(lower, ModelRef {
                provider: normalize_provider_id(&target[..slash]),
                model: target[slash + 1..].to_string(),
            });
        } else {
            // No provider specified, auto-detect
            let provider = super::providers::detect_provider(target);
            aliases.insert(lower, ModelRef {
                provider: provider.to_string(),
                model: target.to_string(),
            });
        }
    }
    aliases
}

// ============================================================================
// Default Model Resolution
// ============================================================================

/// Resolve the default model from config.
pub fn resolve_default_model() -> ModelRef {
    match load_app_config() {
        Ok(cfg) => {
            let model = &cfg.ai_config.model;
            if model.is_empty() {
                return ModelRef {
                    provider: "openai".to_string(),
                    model: "gpt-4o".to_string(),
                };
            }
            resolve_model_ref(model, &HashMap::new())
        }
        Err(_) => ModelRef {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
        },
    }
}

// ============================================================================
// Model Alias Lines (for system prompt)
// ============================================================================

/// Build a formatted list of available model aliases for the system prompt.
pub fn build_model_alias_lines() -> String {
    let mut lines = Vec::new();
    lines.push("Available model shortcuts:".to_string());
    for (alias, provider, model) in builtin_aliases() {
        lines.push(format!("  {} → {}/{}", alias, provider, model));
    }
    lines.join("\n")
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn model_resolve(raw: String) -> Result<ModelRef, String> {
    Ok(resolve_model_ref(&raw, &HashMap::new()))
}

#[tauri::command]
pub async fn model_list_aliases() -> Result<Vec<(String, String, String)>, String> {
    Ok(builtin_aliases()
        .into_iter()
        .map(|(a, p, m)| (a.to_string(), p.to_string(), m.to_string()))
        .collect())
}

#[tauri::command]
pub async fn model_default() -> Result<ModelRef, String> {
    Ok(resolve_default_model())
}
