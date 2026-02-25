//! Messaging — Response chunking, template interpolation, and reply dispatch.
//!
//! Ported from OpenClaw `src/auto-reply/`: handles splitting long AI responses
//! for chat platforms, template variable substitution, and rate-limited dispatch.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageChunk {
    pub index: usize,
    pub total: usize,
    pub content: String,
    pub is_last: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateContext {
    pub body: Option<String>,
    pub sender: Option<String>,
    pub sender_id: Option<String>,
    pub session_key: Option<String>,
    pub channel: Option<String>,
    pub timestamp: Option<String>,
    pub custom: HashMap<String, String>,
}

impl Default for TemplateContext {
    fn default() -> Self {
        Self {
            body: None,
            sender: None,
            sender_id: None,
            session_key: None,
            channel: None,
            timestamp: None,
            custom: HashMap::new(),
        }
    }
}

// ============================================================================
// Response Chunking
// ============================================================================

/// Default max chars per chunk (WeChat limit is ~4096).
const DEFAULT_MAX_CHUNK_SIZE: usize = 3800;

/// Split a long response into chunks, breaking at natural boundaries.
pub fn chunk_response(text: &str, max_size: usize) -> Vec<MessageChunk> {
    let max = if max_size == 0 { DEFAULT_MAX_CHUNK_SIZE } else { max_size };

    if text.len() <= max {
        return vec![MessageChunk {
            index: 0,
            total: 1,
            content: text.to_string(),
            is_last: true,
        }];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max {
            chunks.push(remaining.to_string());
            break;
        }

        // Try to break at natural boundaries (paragraph, sentence, line)
        let slice = &remaining[..max];
        let break_point = find_best_break(slice);
        let (chunk, rest) = remaining.split_at(break_point);
        chunks.push(chunk.to_string());
        remaining = rest.trim_start();
    }

    let total = chunks.len();
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, content)| MessageChunk {
            index: i,
            total,
            content,
            is_last: i == total - 1,
        })
        .collect()
}

/// Find the best break point in text, preferring paragraph > sentence > line > word boundaries.
fn find_best_break(text: &str) -> usize {
    // Try paragraph break (double newline)
    if let Some(pos) = text.rfind("\n\n") {
        if pos > text.len() / 4 {
            return pos + 2;
        }
    }

    // Try sentence break (Chinese/English)
    let sentence_ends = ['。', '！', '？', '.', '!', '?'];
    for &ch in &sentence_ends {
        if let Some(pos) = text.rfind(ch) {
            if pos > text.len() / 4 {
                return pos + ch.len_utf8();
            }
        }
    }

    // Try line break
    if let Some(pos) = text.rfind('\n') {
        if pos > text.len() / 4 {
            return pos + 1;
        }
    }

    // Try word break (space)
    if let Some(pos) = text.rfind(' ') {
        if pos > text.len() / 4 {
            return pos + 1;
        }
    }

    // Last resort: break at max
    text.len()
}

// ============================================================================
// Template Engine
// ============================================================================

/// Apply template interpolation: `{{Variable}}` → value from context.
pub fn apply_template(template: &str, ctx: &TemplateContext) -> String {
    let mut result = template.to_string();

    // Built-in variables
    let now = chrono::Local::now();
    let time_str = now.format("%H:%M:%S").to_string();
    let date_str = now.format("%Y-%m-%d").to_string();
    let datetime_str = now.format("%Y-%m-%d %H:%M:%S").to_string();

    let replacements: Vec<(&str, Option<&str>)> = vec![
        ("Body", ctx.body.as_deref()),
        ("Sender", ctx.sender.as_deref()),
        ("SenderId", ctx.sender_id.as_deref()),
        ("SessionKey", ctx.session_key.as_deref()),
        ("Channel", ctx.channel.as_deref()),
        ("Timestamp", ctx.timestamp.as_deref()),
        ("Time", Some(&time_str)),
        ("Date", Some(&date_str)),
        ("DateTime", Some(&datetime_str)),
    ];

    for (key, value) in replacements {
        let placeholder = format!("{{{{{}}}}}", key);
        let val = value.unwrap_or("");
        result = result.replace(&placeholder, val);
    }

    // Custom variables
    for (key, value) in &ctx.custom {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }

    result
}

// ============================================================================
// Inbound Context Builder
// ============================================================================

/// Build a structured message context from raw inbound data.
pub fn build_inbound_context(
    sender: &str,
    content: &str,
    channel: &str,
    session_key: &str,
) -> TemplateContext {
    TemplateContext {
        body: Some(content.to_string()),
        sender: Some(sender.to_string()),
        sender_id: None,
        session_key: Some(session_key.to_string()),
        channel: Some(channel.to_string()),
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
        custom: HashMap::new(),
    }
}

// ============================================================================
// Reply Rate Limiter
// ============================================================================

use std::sync::atomic::{AtomicU64, Ordering};
use once_cell::sync::Lazy;

static LAST_REPLY_MS: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));

/// Minimum delay between replies in milliseconds (prevents flooding).
const MIN_REPLY_INTERVAL_MS: u64 = 500;

/// Wait if needed to respect rate limits before sending a reply.
pub async fn wait_for_rate_limit() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let last = LAST_REPLY_MS.load(Ordering::Relaxed);
    let elapsed = now.saturating_sub(last);

    if elapsed < MIN_REPLY_INTERVAL_MS {
        let wait = MIN_REPLY_INTERVAL_MS - elapsed;
        tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
    }

    LAST_REPLY_MS.store(now, Ordering::Relaxed);
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn messaging_chunk(text: String, max_size: Option<usize>) -> Result<Vec<MessageChunk>, String> {
    Ok(chunk_response(&text, max_size.unwrap_or(0)))
}

#[tauri::command]
pub async fn messaging_template(template: String, variables: HashMap<String, String>) -> Result<String, String> {
    let ctx = TemplateContext {
        custom: variables,
        ..Default::default()
    };
    Ok(apply_template(&template, &ctx))
}
