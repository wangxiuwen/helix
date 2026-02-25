//! Subagents — Concurrent AI Workers
//!
//! Allows the main agent to spawn isolated, parallel AI tasks.
//! Uses async-openai SDK for AI calls — same as the main agent loop.

use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;

use crate::modules::config::load_app_config;
use super::core::{get_tool_definitions, execute_tool};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentParams {
    pub task: String,
    pub context: Option<String>,
    pub system_prompt: Option<String>,
    pub max_rounds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentResult {
    pub output: String,
    pub rounds_used: u32,
    pub tokens_used: u32,
}

// ============================================================================
// Core Subagent Engine
// ============================================================================

pub async fn run_subagent(params: SubagentParams) -> Result<SubagentResult, String> {
    info!("Starting subagent task: {:?}", &params.task[..params.task.len().min(50)]);

    let config = load_app_config().map_err(|e| format!("config error: {}", e))?;
    let ai = config.ai_config;
    let tool_defs = get_tool_definitions().await;
    let max_rounds = params.max_rounds.unwrap_or(10);

    let openai_config = OpenAIConfig::new()
        .with_api_base(&ai.base_url)
        .with_api_key(&ai.api_key);
    let client = Client::with_config(openai_config);

    let base_prompt = params.system_prompt.unwrap_or_else(|| {
        "你是 Helix 的专属 Subagent (并发执行体)。\n\
         你的工作是独立、专注地完成单个指派任务，并将最终结果汇报给主调用方。\n\n\
         **规则：**\n\
         1. 完整查验：使用提供的工具执行你的任务。如果遇到错误，尝试修复。\n\
         2. 结果导向：一旦你获得了任务所需的最终答案或完成了操作，立即在回复中详细阐述结果。\n\
         3. 如果前置 context 中已经包含了所需信息，你可以直接使用，无需重复调用工具。".to_string()
    });

    let system_prompt = if let Some(ctx) = params.context {
        format!("{}\n\n**背景上下文 (Context)：**\n{}", base_prompt, ctx)
    } else {
        base_prompt
    };

    let mut messages: Vec<ChatCompletionRequestMessage> = vec![
        ChatCompletionRequestSystemMessageArgs::default()
            .content(system_prompt.clone())
            .build()
            .map_err(|e| e.to_string())?
            .into(),
        ChatCompletionRequestUserMessageArgs::default()
            .content(params.task.clone())
            .build()
            .map_err(|e| e.to_string())?
            .into(),
    ];

    let mut rounds_used = 0u32;
    let mut total_tokens = 0u32;

    while rounds_used < max_rounds {
        rounds_used += 1;
        info!("Subagent round {}/{}", rounds_used, max_rounds);

        let request = CreateChatCompletionRequestArgs::default()
            .model(&ai.model)
            .messages(messages.clone())
            .tools(tool_defs.clone())
            .build()
            .map_err(|e| format!("Build request: {}", e))?;

        let response = client
            .chat()
            .create(request)
            .await
            .map_err(|e| format!("AI call failed: {}", e))?;

        if let Some(usage) = &response.usage {
            total_tokens += usage.total_tokens as u32;
        }

        let choice = response.choices.first().ok_or("No choices in response")?;

        if let Some(ref tcs) = choice.message.tool_calls {
            if tcs.is_empty() {
                let final_text = choice.message.content.clone()
                    .unwrap_or_else(|| "Subagent finished with no text response.".to_string());
                return Ok(SubagentResult { output: final_text, rounds_used, tokens_used: total_tokens });
            }

            // Add assistant message with tool calls
            let assistant_msg = ChatCompletionRequestAssistantMessageArgs::default()
                .tool_calls(tcs.clone())
                .build()
                .map_err(|e| e.to_string())?;
            messages.push(assistant_msg.into());

            // Execute tools concurrently
            let mut futures = vec![];
            for tc in tcs {
                let name = tc.function.name.clone();
                let args_str = tc.function.arguments.clone();
                let call_id = tc.id.clone();

                futures.push(async move {
                    let args: Value = serde_json::from_str(&args_str).unwrap_or(serde_json::json!({}));
                    let result = match execute_tool(&name, &args, None).await {
                        Ok(res) => res,
                        Err(e) => format!("Error executing {}: {}", name, e),
                    };
                    (call_id, result)
                });
            }

            let tool_results = futures::future::join_all(futures).await;
            for (call_id, result) in tool_results {
                let tool_msg = ChatCompletionRequestToolMessageArgs::default()
                    .content(result.clone())
                    .tool_call_id(call_id.clone())
                    .build()
                    .map_err(|e| e.to_string())?;
                messages.push(tool_msg.into());
            }

            continue;
        }

        // No tool calls — final answer
        let final_text = choice.message.content.clone()
            .unwrap_or_else(|| "Subagent finished with no text response.".to_string());
        return Ok(SubagentResult { output: final_text, rounds_used, tokens_used: total_tokens });
    }

    Ok(SubagentResult {
        output: "Subagent terminated: reached maximum allowed tool rounds without concluding.".to_string(),
        rounds_used,
        tokens_used: total_tokens,
    })
}

pub async fn run_subagents_batch(tasks: Vec<SubagentParams>) -> Result<Vec<Result<SubagentResult, String>>, String> {
    info!("Spawning {} concurrent subagents", tasks.len());

    let mut handles = vec![];
    for task in tasks {
        let handle = tokio::spawn(async move { run_subagent(task).await });
        handles.push(handle);
    }

    let results = futures::future::join_all(handles).await;
    let mapped: Vec<Result<SubagentResult, String>> = results
        .into_iter()
        .map(|res| match res {
            Ok(sub_res) => sub_res,
            Err(e) => Err(format!("Subagent thread panicked: {}", e)),
        })
        .collect();

    Ok(mapped)
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn spawn_subagent(task: String, context: Option<String>, system_prompt: Option<String>, max_rounds: Option<u32>) -> Result<SubagentResult, String> {
    run_subagent(SubagentParams { task, context, system_prompt, max_rounds }).await
}

#[tauri::command]
pub async fn spawn_subagents_batch(tasks: Vec<SubagentParams>) -> Result<Vec<SubagentResult>, String> {
    let results = run_subagents_batch(tasks).await?;
    let flattened = results.into_iter().map(|r| match r {
        Ok(res) => res,
        Err(e) => SubagentResult {
            output: format!("Subagent Failed: {}", e),
            rounds_used: 0,
            tokens_used: 0,
        }
    }).collect();
    Ok(flattened)
}
