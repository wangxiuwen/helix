//! Streaming Layer — SSE and NDJSON streaming for multi-provider chat completions.
//!
//! Ported from pi-ai `streamSimple()` / `ollama-stream.ts`:
//! unified streaming interface across OpenAI SSE, Anthropic SSE, and Ollama NDJSON.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tracing::info;

use super::providers::{ProviderConfig, ProviderKind, auth_headers, chat_completion_url};

// ============================================================================
// Stream Event Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    /// Incremental text content
    #[serde(rename = "delta")]
    Delta { text: String },
    /// Tool call detected
    #[serde(rename = "tool_call")]
    ToolCallDelta {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments_delta: String,
    },
    /// Usage info (may arrive mid-stream or at end)
    #[serde(rename = "usage")]
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
    },
    /// Stream done
    #[serde(rename = "done")]
    Done {
        stop_reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<StreamUsage>,
    },
    /// Error during streaming
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Accumulated result from a full stream.
#[derive(Debug, Clone)]
pub struct StreamResult {
    pub content: String,
    pub tool_calls: Vec<AccumulatedToolCall>,
    pub usage: StreamUsage,
    pub stop_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccumulatedToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

// ============================================================================
// Unified Streaming Entry Point
// ============================================================================

/// Stream a chat completion and collect all events into a StreamResult.
/// Also calls `on_event` for each event (useful for real-time UI updates).
pub async fn stream_chat_completion(
    provider: &ProviderConfig,
    body: &Value,
    on_event: impl Fn(StreamEvent),
) -> Result<StreamResult, String> {
    match provider.kind {
        ProviderKind::Ollama => stream_ollama(provider, body, on_event).await,
        ProviderKind::Anthropic => stream_anthropic_sse(provider, body, on_event).await,
        _ => stream_openai_sse(provider, body, on_event).await,
    }
}

/// Non-streaming completion (used for simple queries).
pub async fn complete_simple(
    provider: &ProviderConfig,
    body: &Value,
) -> Result<StreamResult, String> {
    let url = chat_completion_url(provider);
    let client = build_client()?;
    let mut request = client.post(&url).timeout(Duration::from_secs(120));

    for (key, val) in auth_headers(provider) {
        request = request.header(&key, &val);
    }

    let resp = request
        .json(body)
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("API error ({}): {}", status, &err[..err.len().min(300)]));
    }

    let data: Value = resp.json().await.map_err(|e| format!("Parse JSON: {}", e))?;

    match provider.kind {
        ProviderKind::Ollama => parse_ollama_response(&data),
        ProviderKind::Anthropic => parse_anthropic_response(&data),
        _ => parse_openai_response(&data),
    }
}

// ============================================================================
// HTTP Client
// ============================================================================

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| format!("Build HTTP client: {}", e))
}

// ============================================================================
// OpenAI SSE Streaming
// ============================================================================

async fn stream_openai_sse(
    provider: &ProviderConfig,
    body: &Value,
    on_event: impl Fn(StreamEvent),
) -> Result<StreamResult, String> {
    let url = chat_completion_url(provider);
    let client = build_client()?;
    let mut request = client.post(&url).timeout(Duration::from_secs(180));

    for (key, val) in auth_headers(provider) {
        request = request.header(&key, &val);
    }

    let resp = request
        .json(body)
        .send()
        .await
        .map_err(|e| format!("SSE request failed: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("API error ({}): {}", status, &err[..err.len().min(300)]));
    }

    let full_text = resp.text().await.map_err(|e| format!("Read SSE body: {}", e))?;

    let mut content = String::new();
    let mut tool_calls: Vec<(String, String, String)> = Vec::new(); // (id, name, args)
    let mut usage = StreamUsage { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 };
    let mut stop_reason = "stop".to_string();

    for line in full_text.lines() {
        let line = line.trim();
        if line.is_empty() || line == "data: [DONE]" {
            continue;
        }
        if !line.starts_with("data: ") {
            continue;
        }

        let json_str = &line[6..];
        let chunk: Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Delta text
        if let Some(delta_content) = chunk["choices"][0]["delta"]["content"].as_str() {
            content.push_str(delta_content);
            on_event(StreamEvent::Delta {
                text: delta_content.to_string(),
            });
        }

        // Tool calls
        if let Some(tc_array) = chunk["choices"][0]["delta"]["tool_calls"].as_array() {
            for tc in tc_array {
                let index = tc["index"].as_u64().unwrap_or(0) as usize;
                let id = tc["id"].as_str().map(|s| s.to_string());
                let name = tc["function"]["name"].as_str().map(|s| s.to_string());
                let args_delta = tc["function"]["arguments"].as_str().unwrap_or("");

                // Grow tool_calls vector as needed
                while tool_calls.len() <= index {
                    tool_calls.push((String::new(), String::new(), String::new()));
                }
                // Only overwrite id/name when non-empty — Qwen sends "" in follow-up chunks
                if let Some(ref id) = id {
                    if !id.is_empty() {
                        tool_calls[index].0 = id.clone();
                    }
                }
                if let Some(ref name) = name {
                    if !name.is_empty() {
                        tool_calls[index].1 = name.clone();
                    }
                }
                tool_calls[index].2.push_str(args_delta);

                on_event(StreamEvent::ToolCallDelta {
                    index,
                    id,
                    name,
                    arguments_delta: args_delta.to_string(),
                });
            }
        }

        // Usage
        if let Some(u) = chunk.get("usage") {
            usage.prompt_tokens = u["prompt_tokens"].as_u64().unwrap_or(0) as u32;
            usage.completion_tokens = u["completion_tokens"].as_u64().unwrap_or(0) as u32;
            usage.total_tokens = u["total_tokens"].as_u64().unwrap_or(0) as u32;
            on_event(StreamEvent::Usage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
            });
        }

        // Stop reason
        if let Some(fr) = chunk["choices"][0]["finish_reason"].as_str() {
            stop_reason = fr.to_string();
        }
    }

    on_event(StreamEvent::Done {
        stop_reason: stop_reason.clone(),
        usage: Some(usage.clone()),
    });

    Ok(StreamResult {
        content,
        tool_calls: {
            // Debug: log raw tool calls before filtering
            for (i, tc) in tool_calls.iter().enumerate() {
                info!("[streaming] raw tool_call[{}]: id='{}' name='{}' args_len={}", i, tc.0, tc.1, tc.2.len());
            }
            let filtered: Vec<AccumulatedToolCall> = tool_calls
                .into_iter()
                .filter(|(id, name, _)| !id.is_empty() || !name.is_empty())
                .map(|(id, name, args)| AccumulatedToolCall { id, name, arguments: args })
                .collect();
            info!("[streaming] filtered tool_calls count: {}", filtered.len());
            filtered
        },
        usage,
        stop_reason,
    })
}

// ============================================================================
// Anthropic SSE Streaming
// ============================================================================

async fn stream_anthropic_sse(
    provider: &ProviderConfig,
    body: &Value,
    on_event: impl Fn(StreamEvent),
) -> Result<StreamResult, String> {
    let url = chat_completion_url(provider);
    let client = build_client()?;
    let mut request = client.post(&url).timeout(Duration::from_secs(180));

    for (key, val) in auth_headers(provider) {
        request = request.header(&key, &val);
    }

    let resp = request
        .json(body)
        .send()
        .await
        .map_err(|e| format!("Anthropic SSE request failed: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("Anthropic API error ({}): {}", status, &err[..err.len().min(300)]));
    }

    let full_text = resp.text().await.map_err(|e| format!("Read Anthropic SSE: {}", e))?;

    let mut content = String::new();
    let mut tool_calls: Vec<AccumulatedToolCall> = Vec::new();
    let mut current_tool_id = String::new();
    let mut current_tool_name = String::new();
    let mut current_tool_args = String::new();
    let mut in_tool_use = false;
    let mut usage = StreamUsage { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 };
    let mut stop_reason = "end_turn".to_string();

    for line in full_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse SSE event type
        if line.starts_with("event: ") {
            let event_type = &line[7..];
            match event_type {
                "content_block_stop" => {
                    if in_tool_use {
                        tool_calls.push(AccumulatedToolCall {
                            id: current_tool_id.clone(),
                            name: current_tool_name.clone(),
                            arguments: current_tool_args.clone(),
                        });
                        in_tool_use = false;
                        current_tool_id.clear();
                        current_tool_name.clear();
                        current_tool_args.clear();
                    }
                }
                _ => {}
            }
            continue;
        }

        if !line.starts_with("data: ") {
            continue;
        }

        let json_str = &line[6..];
        let chunk: Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let chunk_type = chunk["type"].as_str().unwrap_or("");

        match chunk_type {
            "content_block_start" => {
                let block = &chunk["content_block"];
                if block["type"].as_str() == Some("tool_use") {
                    in_tool_use = true;
                    current_tool_id = block["id"].as_str().unwrap_or("").to_string();
                    current_tool_name = block["name"].as_str().unwrap_or("").to_string();
                    current_tool_args.clear();
                }
            }
            "content_block_delta" => {
                let delta = &chunk["delta"];
                if delta["type"].as_str() == Some("text_delta") {
                    let text = delta["text"].as_str().unwrap_or("");
                    content.push_str(text);
                    on_event(StreamEvent::Delta { text: text.to_string() });
                } else if delta["type"].as_str() == Some("input_json_delta") {
                    let partial = delta["partial_json"].as_str().unwrap_or("");
                    current_tool_args.push_str(partial);
                    on_event(StreamEvent::ToolCallDelta {
                        index: tool_calls.len(),
                        id: Some(current_tool_id.clone()),
                        name: Some(current_tool_name.clone()),
                        arguments_delta: partial.to_string(),
                    });
                }
            }
            "message_delta" => {
                if let Some(sr) = chunk["delta"]["stop_reason"].as_str() {
                    stop_reason = sr.to_string();
                }
                if let Some(u) = chunk.get("usage") {
                    usage.completion_tokens = u["output_tokens"].as_u64().unwrap_or(0) as u32;
                    usage.total_tokens = usage.prompt_tokens + usage.completion_tokens;
                }
            }
            "message_start" => {
                if let Some(u) = chunk["message"].get("usage") {
                    usage.prompt_tokens = u["input_tokens"].as_u64().unwrap_or(0) as u32;
                }
            }
            _ => {}
        }
    }

    on_event(StreamEvent::Done {
        stop_reason: stop_reason.clone(),
        usage: Some(usage.clone()),
    });

    Ok(StreamResult {
        content,
        tool_calls,
        usage,
        stop_reason,
    })
}

// ============================================================================
// Ollama NDJSON Streaming
// ============================================================================

async fn stream_ollama(
    provider: &ProviderConfig,
    body: &Value,
    on_event: impl Fn(StreamEvent),
) -> Result<StreamResult, String> {
    let url = chat_completion_url(provider);
    let client = build_client()?;

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(body)
        .timeout(Duration::from_secs(300))
        .send()
        .await
        .map_err(|e| format!("Ollama request failed: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("Ollama error ({}): {}", status, &err[..err.len().min(300)]));
    }

    let full_text = resp.text().await.map_err(|e| format!("Read Ollama NDJSON: {}", e))?;

    let mut content = String::new();
    let mut tool_calls = Vec::new();
    let mut stop_reason = "stop".to_string();
    let mut prompt_eval_count = 0u32;
    let mut eval_count = 0u32;

    for line in full_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let chunk: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Text content
        if let Some(text) = chunk["message"]["content"].as_str() {
            if !text.is_empty() {
                content.push_str(text);
                on_event(StreamEvent::Delta { text: text.to_string() });
            }
        }

        // Tool calls
        if let Some(tcs) = chunk["message"]["tool_calls"].as_array() {
            for tc in tcs {
                let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                let args = tc["function"]["arguments"].clone();
                let args_str = if args.is_object() {
                    serde_json::to_string(&args).unwrap_or_default()
                } else {
                    args.as_str().unwrap_or("{}").to_string()
                };
                tool_calls.push(AccumulatedToolCall {
                    id: format!("ollama_{}", tool_calls.len()),
                    name,
                    arguments: args_str,
                });
            }
        }

        // Done?
        if chunk["done"].as_bool() == Some(true) {
            if let Some(dr) = chunk["done_reason"].as_str() {
                stop_reason = dr.to_string();
            }
            prompt_eval_count = chunk["prompt_eval_count"].as_u64().unwrap_or(0) as u32;
            eval_count = chunk["eval_count"].as_u64().unwrap_or(0) as u32;
        }
    }

    let usage = StreamUsage {
        prompt_tokens: prompt_eval_count,
        completion_tokens: eval_count,
        total_tokens: prompt_eval_count + eval_count,
    };

    on_event(StreamEvent::Done {
        stop_reason: stop_reason.clone(),
        usage: Some(usage.clone()),
    });

    Ok(StreamResult {
        content,
        tool_calls,
        usage,
        stop_reason,
    })
}

// ============================================================================
// Non-streaming Response Parsers
// ============================================================================

fn parse_openai_response(data: &Value) -> Result<StreamResult, String> {
    let choice = &data["choices"][0];
    let message = &choice["message"];

    let content = message["content"].as_str().unwrap_or("").to_string();
    let stop_reason = choice["finish_reason"].as_str().unwrap_or("stop").to_string();

    let tool_calls = if let Some(tcs) = message["tool_calls"].as_array() {
        tcs.iter()
            .filter_map(|tc| {
                Some(AccumulatedToolCall {
                    id: tc["id"].as_str()?.to_string(),
                    name: tc["function"]["name"].as_str()?.to_string(),
                    arguments: tc["function"]["arguments"].as_str().unwrap_or("{}").to_string(),
                })
            })
            .collect()
    } else {
        vec![]
    };

    let usage = if let Some(u) = data.get("usage") {
        StreamUsage {
            prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
        }
    } else {
        StreamUsage { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 }
    };

    Ok(StreamResult { content, tool_calls, usage, stop_reason })
}

fn parse_anthropic_response(data: &Value) -> Result<StreamResult, String> {
    let mut content = String::new();
    let mut tool_calls = Vec::new();

    if let Some(blocks) = data["content"].as_array() {
        for block in blocks {
            match block["type"].as_str() {
                Some("text") => {
                    content.push_str(block["text"].as_str().unwrap_or(""));
                }
                Some("tool_use") => {
                    tool_calls.push(AccumulatedToolCall {
                        id: block["id"].as_str().unwrap_or("").to_string(),
                        name: block["name"].as_str().unwrap_or("").to_string(),
                        arguments: serde_json::to_string(&block["input"]).unwrap_or_default(),
                    });
                }
                _ => {}
            }
        }
    }

    let stop_reason = data["stop_reason"].as_str().unwrap_or("end_turn").to_string();
    let usage = if let Some(u) = data.get("usage") {
        StreamUsage {
            prompt_tokens: u["input_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: u["output_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: (u["input_tokens"].as_u64().unwrap_or(0) + u["output_tokens"].as_u64().unwrap_or(0)) as u32,
        }
    } else {
        StreamUsage { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 }
    };

    Ok(StreamResult { content, tool_calls, usage, stop_reason })
}

fn parse_ollama_response(data: &Value) -> Result<StreamResult, String> {
    let content = data["message"]["content"].as_str().unwrap_or("").to_string();
    let stop_reason = data["done_reason"].as_str().unwrap_or("stop").to_string();

    let tool_calls = if let Some(tcs) = data["message"]["tool_calls"].as_array() {
        tcs.iter().enumerate().map(|(i, tc)| {
            AccumulatedToolCall {
                id: format!("ollama_{}", i),
                name: tc["function"]["name"].as_str().unwrap_or("").to_string(),
                arguments: serde_json::to_string(&tc["function"]["arguments"]).unwrap_or_default(),
            }
        }).collect()
    } else {
        vec![]
    };

    let usage = StreamUsage {
        prompt_tokens: data["prompt_eval_count"].as_u64().unwrap_or(0) as u32,
        completion_tokens: data["eval_count"].as_u64().unwrap_or(0) as u32,
        total_tokens: (data["prompt_eval_count"].as_u64().unwrap_or(0) + data["eval_count"].as_u64().unwrap_or(0)) as u32,
    };

    Ok(StreamResult { content, tool_calls, usage, stop_reason })
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn streaming_test(
    model: String,
    prompt: String,
    base_url: Option<String>,
    api_key: Option<String>,
) -> Result<String, String> {
    let provider = super::providers::resolve_provider_config(
        &model,
        base_url.as_deref(),
        api_key.as_deref(),
        None,
    );

    let body = super::providers::build_openai_request(
        &model,
        &[json!({"role": "user", "content": prompt})],
        None,
        1000,
        false,
    );

    let result = complete_simple(&provider, &body).await?;
    Ok(result.content)
}
