//! Embedded HTTP API Server with Swagger UI.
//!
//! Provides RESTful endpoints for agent chat, tool testing, WeChat messaging,
//! and health checks. Serves Swagger UI at /swagger-ui/.

use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;
use tracing::{info, error};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

use super::agent;
use super::database;

// ============================================================================
// OpenAPI Schema
// ============================================================================

#[derive(OpenApi)]
#[openapi(
    paths(
        health,
        agent_chat,
        tool_web_search,
        tool_web_fetch,
        tool_shell_exec,
        wechat_send,
        wechat_messages,
        wechat_sessions,
    ),
    components(schemas(
        HealthResponse,
        AgentChatRequest,
        AgentChatResponse,
        ToolSearchRequest,
        ToolFetchRequest,
        ToolShellRequest,
        ToolResponse,
        WechatSendRequest,
        WechatSendResponse,
    )),
    tags(
        (name = "health", description = "Health check"),
        (name = "agent", description = "AI Agent chat"),
        (name = "tools", description = "Direct tool invocation"),
        (name = "wechat", description = "WeChat File Transfer Assistant"),
    )
)]
struct ApiDoc;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Serialize, ToSchema)]
struct HealthResponse {
    status: String,
    version: String,
    uptime_secs: u64,
}

#[derive(Deserialize, ToSchema)]
struct AgentChatRequest {
    /// The message to send to the agent
    message: String,
    /// Account/session ID (optional, uses first available if empty)
    #[serde(default)]
    account_id: String,
}

#[derive(Serialize, ToSchema)]
struct AgentChatResponse {
    reply: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Deserialize, ToSchema)]
struct ToolSearchRequest {
    /// Search query
    query: String,
    /// Number of results (default: 5)
    #[serde(default)]
    num_results: Option<u64>,
}

#[derive(Deserialize, ToSchema)]
struct ToolFetchRequest {
    /// URL to fetch
    url: String,
    /// HTTP method (GET, POST, etc.)
    #[serde(default)]
    method: Option<String>,
}

#[derive(Deserialize, ToSchema)]
struct ToolShellRequest {
    /// Shell command to execute
    command: String,
    /// Working directory
    #[serde(default)]
    working_dir: Option<String>,
    /// Timeout in seconds
    #[serde(default)]
    timeout_secs: Option<u64>,
}

#[derive(Serialize, ToSchema)]
struct ToolResponse {
    success: bool,
    result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Deserialize, ToSchema)]
struct WechatSendRequest {
    /// Session ID
    session_id: String,
    /// Message content
    content: String,
}

#[derive(Serialize, ToSchema)]
struct WechatSendResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// ============================================================================
// Endpoints
// ============================================================================

/// Health check
#[utoipa::path(
    get, path = "/api/health",
    tag = "health",
    responses((status = 200, description = "Server is healthy", body = HealthResponse))
)]
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs: 0, // TODO: track actual uptime
    })
}

/// Send a message to the AI agent and get a response
#[utoipa::path(
    post, path = "/api/agent/chat",
    tag = "agent",
    request_body = AgentChatRequest,
    responses(
        (status = 200, description = "Agent response", body = AgentChatResponse),
        (status = 500, description = "Agent error", body = AgentChatResponse),
    )
)]
async fn agent_chat(Json(req): Json<AgentChatRequest>) -> impl IntoResponse {
    let account_id = if req.account_id.is_empty() {
        // Try to find first available account
        match database::list_accounts() {
            Ok(accounts) if !accounts.is_empty() => accounts[0].id.clone(),
            _ => "api-test".to_string(),
        }
    } else {
        req.account_id
    };

    info!("[API] agent_chat: account={}, msg={}", account_id, &req.message);

    match agent::agent_process_message(&account_id, &req.message).await {
        Ok(reply) => (
            StatusCode::OK,
            Json(AgentChatResponse { reply, error: None }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AgentChatResponse {
                reply: String::new(),
                error: Some(e),
            }),
        ),
    }
}

/// Search the web using the web_search tool
#[utoipa::path(
    post, path = "/api/tools/web_search",
    tag = "tools",
    request_body = ToolSearchRequest,
    responses(
        (status = 200, description = "Search results", body = ToolResponse),
    )
)]
async fn tool_web_search(Json(req): Json<ToolSearchRequest>) -> Json<ToolResponse> {
    let args = json!({
        "query": req.query,
        "num_results": req.num_results.unwrap_or(5),
    });

    info!("[API] web_search: query={}", req.query);

    match agent::execute_tool("web_search", &args, None).await {
        Ok(result) => Json(ToolResponse {
            success: true,
            result,
            error: None,
        }),
        Err(e) => Json(ToolResponse {
            success: false,
            result: String::new(),
            error: Some(e),
        }),
    }
}

/// Fetch content from a URL
#[utoipa::path(
    post, path = "/api/tools/web_fetch",
    tag = "tools",
    request_body = ToolFetchRequest,
    responses(
        (status = 200, description = "Fetched content", body = ToolResponse),
    )
)]
async fn tool_web_fetch(Json(req): Json<ToolFetchRequest>) -> Json<ToolResponse> {
    let args = json!({
        "url": req.url,
        "method": req.method.unwrap_or_else(|| "GET".to_string()),
    });

    info!("[API] web_fetch: url={}", req.url);

    match agent::execute_tool("web_fetch", &args, None).await {
        Ok(result) => Json(ToolResponse {
            success: true,
            result,
            error: None,
        }),
        Err(e) => Json(ToolResponse {
            success: false,
            result: String::new(),
            error: Some(e),
        }),
    }
}

/// Execute a shell command
#[utoipa::path(
    post, path = "/api/tools/shell_exec",
    tag = "tools",
    request_body = ToolShellRequest,
    responses(
        (status = 200, description = "Command output", body = ToolResponse),
    )
)]
async fn tool_shell_exec(Json(req): Json<ToolShellRequest>) -> Json<ToolResponse> {
    let args = json!({
        "command": req.command,
        "working_dir": req.working_dir.unwrap_or_else(|| "~".to_string()),
        "timeout_secs": req.timeout_secs.unwrap_or(30),
    });

    info!("[API] shell_exec: cmd={}", req.command);

    match agent::execute_tool("shell_exec", &args, None).await {
        Ok(result) => Json(ToolResponse {
            success: true,
            result,
            error: None,
        }),
        Err(e) => Json(ToolResponse {
            success: false,
            result: String::new(),
            error: Some(e),
        }),
    }
}

/// Send a message via WeChat File Transfer Assistant
#[utoipa::path(
    post, path = "/api/wechat/send",
    tag = "wechat",
    request_body = WechatSendRequest,
    responses(
        (status = 200, description = "Message sent", body = WechatSendResponse),
    )
)]
async fn wechat_send(Json(req): Json<WechatSendRequest>) -> Json<WechatSendResponse> {
    info!("[API] wechat_send: session={}, content_len={}", req.session_id, req.content.len());

    match super::filehelper::send_text_message(&req.session_id, &req.content, false).await {
        Ok(_) => Json(WechatSendResponse { ok: true, error: None }),
        Err(e) => Json(WechatSendResponse { ok: false, error: Some(e) }),
    }
}

/// Get message history for a WeChat session
#[utoipa::path(
    get, path = "/api/wechat/messages",
    tag = "wechat",
    params(
        ("session_id" = String, Query, description = "WeChat session ID"),
        ("limit" = Option<i64>, Query, description = "Max messages to return"),
    ),
    responses(
        (status = 200, description = "Message list", body = Value),
    )
)]
async fn wechat_messages(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let session_id = params.get("session_id").cloned().unwrap_or_default();
    let limit = params.get("limit")
        .and_then(|l| l.parse::<i64>().ok())
        .unwrap_or(50);

    match database::get_messages(&session_id, limit, 0) {
        Ok(msgs) => Json(json!({ "messages": msgs, "count": msgs.len() })),
        Err(e) => Json(json!({ "error": e, "messages": [] })),
    }
}

/// List all WeChat sessions
#[utoipa::path(
    get, path = "/api/wechat/sessions",
    tag = "wechat",
    responses(
        (status = 200, description = "Session list", body = Value),
    )
)]
async fn wechat_sessions() -> Json<Value> {
    match super::filehelper::filehelper_list_sessions().await {
        Ok(v) => Json(v),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ============================================================================
// Server Startup
// ============================================================================

/// Start the embedded API server on the given port.
pub fn start_api_server(port: u16) {
    info!("Starting API server on port {}", port);

    tauri::async_runtime::spawn(async move {
        let app = Router::new()
            // Health
            .route("/api/health", get(health))
            // Agent
            .route("/api/agent/chat", post(agent_chat))
            // Tools
            .route("/api/tools/web_search", post(tool_web_search))
            .route("/api/tools/web_fetch", post(tool_web_fetch))
            .route("/api/tools/shell_exec", post(tool_shell_exec))
            // WeChat
            .route("/api/wechat/send", post(wechat_send))
            .route("/api/wechat/messages", get(wechat_messages))
            .route("/api/wechat/sessions", get(wechat_sessions))
            // Swagger UI
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
            // CORS
            .layer(CorsLayer::permissive());

        let addr = format!("0.0.0.0:{}", port);
        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to bind API server to {}: {}", addr, e);
                return;
            }
        };

        info!("✅ API server listening on http://localhost:{}", port);
        info!("📖 Swagger UI: http://localhost:{}/swagger-ui/", port);

        if let Err(e) = axum::serve(listener, app).await {
            error!("API server error: {}", e);
        }
    });
}
