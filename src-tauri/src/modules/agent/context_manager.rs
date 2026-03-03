use agents_sdk::messaging::{AgentMessage, MessageContent, MessageRole};
use serde_json::Value;
use tracing::{info, warn};

// Estimations
const CHARS_PER_TOKEN: usize = 3;

// Layer 1
const TOOL_PROTECTION_THRESHOLD: usize = 30000;
const MIN_PRUNABLE_THRESHOLD: usize = 15000;
const MAX_TOOL_RESULT_CHARS: usize = 8000;
const PREVIEW_HEAD_CHARS: usize = 500;
const PREVIEW_TAIL_CHARS: usize = 500;

// Layer 2
const COMPRESSION_TOKEN_THRESHOLD: f64 = 0.45;
const COMPRESSION_PRESERVE_RATIO: f64 = 0.35;

// Layer 3
const OVERFLOW_SAFETY_MARGIN: f64 = 0.85;

pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    (text.len() as f64 / CHARS_PER_TOKEN as f64).ceil() as usize
}

pub fn estimate_message_tokens(msg: &AgentMessage) -> usize {
    let mut text_len = 0;
    match &msg.content {
        MessageContent::Text(t) => text_len += t.len(),
        MessageContent::Json(v) => text_len += v.to_string().len(),
    }
    (text_len as f64 / CHARS_PER_TOKEN as f64).ceil() as usize
}

pub fn estimate_messages_tokens(messages: &[AgentMessage]) -> usize {
    messages.iter().map(estimate_message_tokens).sum()
}

// ----------------------------------------------------------------------------
// Layer 1: Tool Output Masking
// ----------------------------------------------------------------------------
pub fn mask_tool_outputs(messages: &[AgentMessage]) -> Vec<AgentMessage> {
    let mut accumulated_tool_tokens = 0;
    let mut prunable_tokens = 0;
    
    // Track pairs of (index, original_text, tokens)
    let mut items_to_prune: Vec<(usize, String, usize)> = Vec::new();
    let mut new_messages = messages.to_vec();

    // Reverse scan
    for (i, msg) in new_messages.iter().enumerate().rev() {
        if msg.role == MessageRole::Tool {
            let content_str = match &msg.content {
                MessageContent::Text(t) => t.clone(),
                MessageContent::Json(v) => v.to_string(),
            };
            
            let tokens = estimate_tokens(&content_str);

            if accumulated_tool_tokens < TOOL_PROTECTION_THRESHOLD {
                accumulated_tool_tokens += tokens;
            } else if content_str.len() > MAX_TOOL_RESULT_CHARS {
                prunable_tokens += tokens;
                items_to_prune.push((i, content_str.clone(), tokens));
            }
        }
    }

    if prunable_tokens >= MIN_PRUNABLE_THRESHOLD {
        for (idx, original_content, _) in items_to_prune.iter() {
            let head: String = original_content.chars().take(PREVIEW_HEAD_CHARS).collect();
            let tail_start = original_content.chars().count().saturating_sub(PREVIEW_TAIL_CHARS);
            let tail: String = original_content.chars().skip(tail_start).collect();
            
            let omitted_lines = original_content.lines().count().saturating_sub(
                head.lines().count() + tail.lines().count()
            );
            
            let approx_kb = original_content.len() / 1024;
            
            let replacement = format!(
                "[Tool output truncated — original: ~{} lines, ~{}KB]\n{}\n...\n[{} lines omitted]\n...\n{}",
                omitted_lines, approx_kb, head, omitted_lines, tail
            );
            
            new_messages[*idx].content = MessageContent::Text(replacement);
        }
        
        info!("[ContextManager] Masked {} tool outputs, saved ~{} tokens.", items_to_prune.len(), prunable_tokens);
    }

    new_messages
}

// ----------------------------------------------------------------------------
// Layer 2: Chat Compression (LLM call omitted for wrapper simplicity, could be added later if needed)
// ----------------------------------------------------------------------------
// Note: Chat compaction is already handled in the background by memory.rs in Helix.
// The true 3-Layer compression requires interrupting the SDK loop, 
// but for now, we will focus on Masking and Overflow clipping which are 100% reliable.

// ----------------------------------------------------------------------------
// Layer 3: Overflow Prevention
// ----------------------------------------------------------------------------
pub struct OverflowStatus {
    pub safe: bool,
    pub total_tokens: usize,
    pub limit: usize,
    pub usage_percent: usize,
}

pub fn check_overflow(messages: &[AgentMessage], context_limit: usize) -> OverflowStatus {
    // Assuming context Limit defaults to 131072 if not provided exactly
    let limit = if context_limit == 0 { 131072 } else { context_limit };
    let total_tokens = estimate_messages_tokens(messages);
    
    let hard_limit = (limit as f64 * OVERFLOW_SAFETY_MARGIN) as usize;
    let usage_percent = ((total_tokens as f64 / limit as f64) * 100.0).round() as usize;

    OverflowStatus {
        safe: total_tokens < hard_limit,
        total_tokens,
        limit,
        usage_percent,
    }
}

/// Helper method for InterceptingChatModel
pub fn emergency_trim(messages: &mut Vec<AgentMessage>) {
    if messages.len() > 10 {
        // Keep system prompt + the last 30% of messages
        let retain_len = (messages.len() as f64 * 0.3).ceil() as usize;
        let start_idx = messages.len() - retain_len;
        
        let mut trimmed = Vec::new();
        // Assume first might be System
        if !messages.is_empty() && messages[0].role == MessageRole::System {
            trimmed.push(messages[0].clone());
        }
        
            trimmed.push(AgentMessage {
                role: MessageRole::System,
                content: MessageContent::Text("[Earlier context was truncated due to context window overflow]".to_string()),
                metadata: None,
            });
        
        let slice = &messages.clone()[start_idx..];
        for m in slice {
            trimmed.push(m.clone());
        }
        
        *messages = trimmed;
        warn!("[ContextManager] Emergency trim applied due to overflow.");
    }
}
