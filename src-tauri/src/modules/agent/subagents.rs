//! Subagents — Concurrent AI Workers powered by agents-sdk.
//!
//! Allows the main agent to spawn isolated, parallel AI tasks.

use agents_sdk::{ConfigurableAgentBuilder, OpenAiConfig, OpenAiChatModel, persistence::InMemoryCheckpointer};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::info;

use crate::modules::config::load_app_config;

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
// Core Subagent Engine — powered by agents-sdk
// ============================================================================

pub async fn run_subagent(params: SubagentParams) -> Result<SubagentResult, String> {
    info!("Starting subagent task: {:?}", &params.task[..params.task.len().min(50)]);

    let config = load_app_config().map_err(|e| format!("config error: {}", e))?;
    let ai = config.ai_config;

    let full_api_url = format!("{}/chat/completions", ai.base_url.trim_end_matches('/'));
    let oai_config = OpenAiConfig::new(&ai.api_key, &ai.model)
        .with_api_url(Some(full_api_url));
    let model = Arc::new(
        OpenAiChatModel::new(oai_config).map_err(|e| format!("Model init: {}", e))?
    );

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

    let sdk_tools = super::tools::build_tools();

    let agent = ConfigurableAgentBuilder::new("Helix Subagent")
        .with_model(model)
        .with_system_prompt(&system_prompt)
        .with_tools(sdk_tools)
        .with_checkpointer(Arc::new(InMemoryCheckpointer::new()))
        .build()
        .map_err(|e| format!("Agent build: {}", e))?;

    let state = Arc::new(agents_sdk::state::AgentStateSnapshot::default());
    let response = agent.handle_message(&params.task, state).await
        .map_err(|e| format!("Subagent error: {}", e))?;

    let output = match &response.content {
        agents_sdk::messaging::MessageContent::Text(t) => t.clone(),
        other => format!("{:?}", other),
    };

    Ok(SubagentResult {
        output,
        rounds_used: 1,  // SDK handles rounds internally
        tokens_used: 0,   // SDK doesn't expose token count yet
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
