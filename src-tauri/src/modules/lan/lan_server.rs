use axum::{
    extract::{State, Json},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;
use tauri::Emitter;

#[derive(Clone)]
pub struct AppState {
    pub app_handle: Option<tauri::AppHandle>,
}

#[derive(Deserialize)]
pub struct IncomingMessage {
    pub session_id: String,
    pub role: String,
    pub name: String,
    pub content: String,
    pub reply_to: Option<String>,
}

pub async fn start_lan_server(app_handle: Option<tauri::AppHandle>, port: u16) -> anyhow::Result<()> {
    let state = AppState { app_handle };

    let app = Router::new()
        .route("/api/localsend/v2/info", get(info_handler))
        .route("/api/helix/v1/message", post(message_handler))
        .with_state(state);

    let bind_str = format!("0.0.0.0:{}", port);
    
    tokio::spawn(async move {
        match tokio::net::TcpListener::bind(&bind_str).await {
            Ok(listener) => {
                info!("LAN HTTP Server listening on {}", bind_str);
                if let Err(e) = axum::serve(listener, app).await {
                    tracing::error!("LAN server error: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("LAN server bind error on {}: {}", bind_str, e);
            }
        }
    });

    Ok(())
}

async fn info_handler() -> Json<Value> {
    Json(json!({
        "alias": "Helix Node",
        "version": "2.0",
        "deviceModel": "Desktop",
        "deviceType": "desktop",
        "fingerprint": "standalone",
        "download": true
    }))
}

async fn message_handler(
    State(state): State<AppState>,
    Json(payload): Json<IncomingMessage>,
) -> Json<Value> {
    if let Some(app) = state.app_handle {
        let _ = app.emit("lan-message-received", json!({
            "session_id": payload.session_id,
            "role": payload.role,
            "name": payload.name,
            "content": payload.content,
            "reply_to": payload.reply_to,
        }));
    }
    Json(json!({"success": true}))
}
