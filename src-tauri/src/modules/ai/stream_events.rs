//! Stream Events — Tauri event emission and thinking/reasoning tag stripping.
//!
//! Ported from OpenClaw `pi-embedded-subscribe.ts`:
//! - Strips `<thinking>`, `<thought>`, `<antthinking>` reasoning tags
//! - Strips `<final>` tags (extracts inner content)
//! - Emits real-time Tauri events for streaming UI updates
//! - Block reply chunking for progressive display

use regex::Regex;
use serde::{Deserialize, Serialize};
use once_cell::sync::Lazy;

// ============================================================================
// Tauri Event Names
// ============================================================================

/// Event names emitted to the frontend.
pub const EVENT_STREAM_DELTA: &str = "agent:stream:delta";
pub const EVENT_STREAM_DONE: &str = "agent:stream:done";
pub const EVENT_TOOL_START: &str = "agent:tool:start";
pub const EVENT_TOOL_RESULT: &str = "agent:tool:result";
pub const EVENT_THINKING: &str = "agent:thinking";
pub const EVENT_ERROR: &str = "agent:error";

// ============================================================================
// Event Payload Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDeltaPayload {
    pub text: String,
    pub accumulated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDonePayload {
    pub content: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub tool_calls_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStartPayload {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultPayload {
    pub id: String,
    pub name: String,
    pub result: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingPayload {
    pub text: String,
    pub is_start: bool,
}

// ============================================================================
// Thinking / Reasoning Tag Stripping
// ============================================================================

/// Regex patterns for thinking tags (Anthropic's <thinking>, <thought>, <antthinking>).
static THINKING_TAG_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?si)<\s*/?\s*(?:think(?:ing)?|thought|antthinking)\s*>").unwrap()
});

/// Regex for <final> tags.
static FINAL_TAG_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?si)<\s*/?\s*final\s*>").unwrap()
});

/// Regex for thinking block content (matches entire block).
static THINKING_BLOCK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?si)<\s*(?:think(?:ing)?|thought|antthinking)\s*>.*?<\s*/\s*(?:think(?:ing)?|thought|antthinking)\s*>").unwrap()
});

/// Regex for final block (extracts inner content).
static FINAL_BLOCK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?si)<\s*final\s*>(.*?)<\s*/\s*final\s*>").unwrap()
});

/// Strip all thinking/reasoning blocks from text.
/// `<thinking>internal reasoning</thinking>` → removed entirely.
pub fn strip_thinking_blocks(text: &str) -> String {
    let result = THINKING_BLOCK_RE.replace_all(text, "");
    // Also strip any orphaned opening/closing tags
    let result = THINKING_TAG_RE.replace_all(&result, "");
    result.trim().to_string()
}

/// Process final tags: extract content from `<final>...</final>` blocks.
/// If no `<final>` tags, returns the text as-is (with thinking stripped).
pub fn process_final_tags(text: &str) -> String {
    // First strip thinking blocks
    let cleaned = strip_thinking_blocks(text);

    // Check for <final> blocks
    if let Some(caps) = FINAL_BLOCK_RE.captures(&cleaned) {
        if let Some(inner) = caps.get(1) {
            return inner.as_str().trim().to_string();
        }
    }

    // No <final> tags, just strip any orphaned final tags
    FINAL_TAG_RE.replace_all(&cleaned, "").trim().to_string()
}

/// Full pipeline: strip thinking + process final tags.
/// Used for the final response before sending to user.
pub fn clean_response(text: &str) -> String {
    process_final_tags(text)
}

// ============================================================================
// Streaming State Tracker
// ============================================================================

/// Tracks streaming state for progressive tag detection.
pub struct StreamProcessor {
    /// Accumulated full text (before stripping)
    pub raw_buffer: String,
    /// Whether we're currently inside a thinking block
    pub in_thinking: bool,
    /// Accumulated thinking content (for debugging)
    pub thinking_buffer: String,
    /// Whether we've seen a <final> opening tag
    pub in_final: bool,
    /// Content inside <final> block
    pub final_buffer: String,
    /// Cleaned output text (thinking stripped, final extracted)
    pub output_buffer: String,
}

impl StreamProcessor {
    pub fn new() -> Self {
        Self {
            raw_buffer: String::new(),
            in_thinking: false,
            thinking_buffer: String::new(),
            in_final: false,
            final_buffer: String::new(),
            output_buffer: String::new(),
        }
    }

    /// Process a streaming delta chunk.
    /// Returns the cleaned text that should be shown to the user (may be empty if in thinking block).
    pub fn process_delta(&mut self, delta: &str) -> ProcessedDelta {
        self.raw_buffer.push_str(delta);

        let mut visible_text = String::new();
        let mut thinking_text = String::new();
        let mut entered_thinking = false;
        let mut exited_thinking = false;

        // Simple state machine for streaming tag detection
        for ch in delta.chars() {
            self.output_buffer.push(ch);

            // Check for tag transitions at the end of the buffer
            let recent = if self.output_buffer.len() > 50 {
                &self.output_buffer[self.output_buffer.len() - 50..]
            } else {
                &self.output_buffer
            };

            // Detect opening thinking tag
            if !self.in_thinking && self.check_thinking_open(recent) {
                self.in_thinking = true;
                entered_thinking = true;
                // Remove the tag from output
                self.trim_last_tag_from_output();
                continue;
            }

            // Detect closing thinking tag
            if self.in_thinking && self.check_thinking_close(recent) {
                self.in_thinking = false;
                exited_thinking = true;
                self.trim_last_tag_from_output();
                continue;
            }

            // Detect <final> open
            if !self.in_final && self.check_final_open(recent) {
                self.in_final = true;
                self.trim_last_tag_from_output();
                continue;
            }

            // Detect </final> close
            if self.in_final && self.check_final_close(recent) {
                self.in_final = false;
                self.trim_last_tag_from_output();
                continue;
            }

            if self.in_thinking {
                thinking_text.push(ch);
                self.thinking_buffer.push(ch);
            } else {
                visible_text.push(ch);
            }
        }

        ProcessedDelta {
            visible_text,
            thinking_text,
            entered_thinking,
            exited_thinking,
        }
    }

    fn check_thinking_open(&self, recent: &str) -> bool {
        let lower = recent.to_lowercase();
        lower.ends_with("<thinking>")
            || lower.ends_with("<thought>")
            || lower.ends_with("<antthinking>")
    }

    fn check_thinking_close(&self, recent: &str) -> bool {
        let lower = recent.to_lowercase();
        lower.ends_with("</thinking>")
            || lower.ends_with("</thought>")
            || lower.ends_with("</antthinking>")
    }

    fn check_final_open(&self, recent: &str) -> bool {
        recent.to_lowercase().ends_with("<final>")
    }

    fn check_final_close(&self, recent: &str) -> bool {
        recent.to_lowercase().ends_with("</final>")
    }

    fn trim_last_tag_from_output(&mut self) {
        // Remove the last tag-like content from output_buffer
        if let Some(pos) = self.output_buffer.rfind('<') {
            self.output_buffer.truncate(pos);
        }
    }

    /// Get the final cleaned response.
    pub fn finalize(&self) -> String {
        clean_response(&self.raw_buffer)
    }
}

#[derive(Debug, Clone)]
pub struct ProcessedDelta {
    /// Text visible to the user
    pub visible_text: String,
    /// Text that was inside thinking tags (hidden)
    pub thinking_text: String,
    /// Whether we just entered a thinking block
    pub entered_thinking: bool,
    /// Whether we just exited a thinking block
    pub exited_thinking: bool,
}

// ============================================================================
// Block Reply Chunking
// ============================================================================

/// Split a streaming response into block-reply chunks for real-time delivery.
/// Returns completed paragraphs that can be sent immediately.
pub fn extract_complete_blocks(text: &str) -> (Vec<String>, String) {
    let mut blocks = Vec::new();
    let remainder;

    // Split on double newlines (paragraph boundaries)
    let parts: Vec<&str> = text.split("\n\n").collect();

    if parts.len() <= 1 {
        // No complete paragraph boundary yet
        return (blocks, text.to_string());
    }

    // All parts except the last one are complete paragraphs
    for part in &parts[..parts.len() - 1] {
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            blocks.push(trimmed.to_string());
        }
    }

    // Last part is potentially incomplete
    remainder = parts.last().unwrap_or(&"").to_string();

    (blocks, remainder)
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn stream_clean_text(text: String) -> Result<String, String> {
    Ok(clean_response(&text))
}

#[tauri::command]
pub async fn stream_strip_thinking(text: String) -> Result<String, String> {
    Ok(strip_thinking_blocks(&text))
}
