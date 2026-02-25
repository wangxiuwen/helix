//! Telegram Bot API compatible HTTP endpoints for WeChat File Helper.
//!
//! Provides a standard Telegram Bot API interface so external tools
//! (e.g. filehelper_sdk.py, curl, or any TG bot client) can interact
//! with the WeChat File Transfer Assistant through familiar endpoints.
//!
//! Endpoints ported from wx-filehelper-api Python project:
//! - TG Bot API: getMe, getUpdates, sendMessage, sendDocument, sendPhoto, getFile, getChat, webhook
//! - File management: /files/metadata, /files/delete, /files/cleanup
//! - Store: /store/stats, /store/messages
//! - Convenience: /health, /login/status, /qr, /send, /messages

use axum::{
    extract::{Json, Multipart, Query},
    routing::{get, post, delete},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use tracing::{info, warn, error};

use crate::modules::database;
use super::{SESSIONS, build_client, build_no_redirect_client, FileHelperSession};
use super::protocol;
use super::commands as fh_commands;

// ============================================================================
// Webhook state
// ============================================================================

static WEBHOOK_URL: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

fn set_webhook_url(url: Option<String>) {
    let mut wh = WEBHOOK_URL.lock().unwrap();
    *wh = url;
}

fn get_webhook_url() -> Option<String> {
    WEBHOOK_URL.lock().unwrap().clone()
}

/// Public getter for other modules (e.g. bot_commands settings display)
pub fn get_webhook_url_public() -> Option<String> {
    get_webhook_url()
}

/// Push a message to the configured webhook URL (fire-and-forget).
pub fn push_to_webhook(update: Value) {
    let url = match get_webhook_url() {
        Some(u) if !u.is_empty() => u,
        _ => return,
    };

    tokio::spawn(async move {
        let client = reqwest::Client::new();
        if let Err(e) = client.post(&url).json(&update).send().await {
            warn!("[Webhook] push failed: {}", e);
        }
    });
}

// ============================================================================
// Helper: resolve first active session
// ============================================================================

fn first_session_id() -> Result<String, String> {
    let sessions = SESSIONS.lock().unwrap();
    sessions.values()
        .find(|s| s.session.logged_in)
        .map(|s| s.id.clone())
        .ok_or_else(|| "No active WeChat session. Please scan QR code first.".to_string())
}

fn ensure_session_id() -> String {
    let sessions = SESSIONS.lock().unwrap();
    if let Some(s) = sessions.values().find(|s| s.session.logged_in) {
        return s.id.clone();
    }
    if let Some(s) = sessions.values().next() {
        return s.id.clone();
    }
    drop(sessions);
    let id = uuid::Uuid::new_v4().to_string();
    let mut sessions = SESSIONS.lock().unwrap();
    sessions.insert(id.clone(), super::SessionState {
        id: id.clone(),
        session: FileHelperSession::default(),
        login_uuid: String::new(),
        client: build_client(),
        no_redirect_client: build_no_redirect_client(),
        is_polling: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        last_msg_count: 0,
    });
    id
}

// ============================================================================
// Request types
// ============================================================================

#[derive(Deserialize, Default)]
struct GetUpdatesQuery {
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Deserialize)]
struct SendMessageBody {
    text: String,
    #[serde(default)]
    session_id: Option<String>,
}

#[derive(Deserialize)]
struct SendDocumentBody {
    file_path: String,
    #[serde(default)]
    session_id: Option<String>,
}

#[derive(Deserialize)]
struct SetWebhookBody {
    url: String,
}

#[derive(Deserialize, Default)]
struct SendQuery {
    text: Option<String>,
    session_id: Option<String>,
}

#[derive(Deserialize, Default)]
struct MessagesQuery {
    session_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize, Default)]
struct LoginStatusQuery {
    session_id: Option<String>,
}

#[derive(Deserialize, Default)]
struct QrQuery {
    session_id: Option<String>,
}

#[derive(Deserialize, Default)]
struct GetFileQuery {
    file_id: Option<String>,
}

#[derive(Deserialize, Default)]
struct FilesQuery {
    session_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize, Default)]
struct CleanupQuery {
    session_id: Option<String>,
    days: Option<i64>,
}

#[derive(Deserialize, Default)]
struct StoreMessagesQuery {
    session_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

// ============================================================================
// TG Bot API endpoints
// ============================================================================

async fn get_me() -> Json<Value> {
    let session_id = first_session_id().ok();
    let (logged_in, username) = session_id.map(|sid| {
        let sessions = SESSIONS.lock().unwrap();
        sessions.get(&sid)
            .map(|s| (s.session.logged_in, s.session.username.clone()))
            .unwrap_or((false, String::new()))
    }).unwrap_or((false, String::new()));

    Json(json!({
        "ok": true,
        "result": {
            "id": "filehelper",
            "is_bot": true,
            "first_name": if username.is_empty() { "WeChat FileHelper".to_string() } else { username },
            "username": "filehelper",
            "is_logged_in": logged_in,
        }
    }))
}

async fn get_updates(Query(q): Query<GetUpdatesQuery>) -> Json<Value> {
    let session_id = match first_session_id() {
        Ok(id) => id,
        Err(e) => return Json(json!({ "ok": false, "error": e })),
    };

    let offset = q.offset.unwrap_or(0);
    let limit = q.limit.unwrap_or(100).min(1000);

    match database::get_updates(&session_id, offset, limit) {
        Ok(messages) => {
            let updates: Vec<Value> = messages.iter().map(|msg| {
                json!({
                    "update_id": msg.id,
                    "message": {
                        "message_id": msg.id.to_string(),
                        "date": msg.created_at,
                        "text": msg.content,
                        "from": {
                            "id": if msg.ai_reply { "bot" } else { "user" },
                            "is_bot": msg.ai_reply,
                        },
                        "chat": { "id": "filehelper", "type": "private" }
                    }
                })
            }).collect();

            Json(json!({ "ok": true, "result": updates }))
        }
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

async fn send_message(Json(body): Json<SendMessageBody>) -> Json<Value> {
    let session_id = body.session_id.clone()
        .or_else(|| first_session_id().ok())
        .unwrap_or_default();

    if session_id.is_empty() {
        return Json(json!({ "ok": false, "error": "No active session" }));
    }

    info!("[BotAPI] sendMessage: session={}, len={}", session_id, body.text.len());

    match protocol::send_text_message(&session_id, &body.text, false).await {
        Ok(_) => {
            let msg_id = database::save_message(&session_id, &body.text, true, 1, false).unwrap_or(0);

            // Push to webhook if configured
            push_to_webhook(json!({
                "update_id": msg_id,
                "message": {
                    "message_id": msg_id, "text": &body.text,
                    "date": chrono::Utc::now().timestamp(),
                    "from": { "id": "self", "is_bot": false },
                    "chat": { "id": "filehelper", "type": "private" },
                }
            }));

            Json(json!({
                "ok": true,
                "result": {
                    "message_id": msg_id,
                    "text": body.text,
                    "date": chrono::Utc::now().timestamp(),
                }
            }))
        }
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

async fn send_document_json(Json(body): Json<SendDocumentBody>) -> Json<Value> {
    let session_id = body.session_id.clone()
        .or_else(|| first_session_id().ok())
        .unwrap_or_default();

    if session_id.is_empty() {
        return Json(json!({ "ok": false, "error": "No active session" }));
    }

    let path = std::path::Path::new(&body.file_path);
    if !path.exists() {
        return Json(json!({ "ok": false, "error": "File not found" }));
    }

    info!("[BotAPI] sendDocument(json): session={}, path={}", session_id, body.file_path);

    match protocol::send_file_to_wechat(&session_id, &body.file_path).await {
        Ok(result) => {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
            let file_size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);

            // Save file metadata to DB
            let _ = database::save_file(&session_id, None, file_name, &body.file_path, file_size, None);

            Json(json!({
                "ok": true,
                "result": {
                    "message_id": format!("file_{}", chrono::Utc::now().timestamp_millis()),
                    "document": { "file_name": file_name, "file_path": body.file_path },
                    "detail": result,
                }
            }))
        }
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// Handle multipart file upload for sendDocument/sendPhoto
async fn send_document_upload(mut multipart: Multipart) -> Json<Value> {
    let session_id = match first_session_id() {
        Ok(id) => id,
        Err(e) => return Json(json!({ "ok": false, "error": e })),
    };

    let mut file_name = String::from("upload");
    let mut file_data: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "document" || name == "photo" || name == "file" {
            file_name = field.file_name().unwrap_or("upload").to_string();
            match field.bytes().await {
                Ok(bytes) => { file_data = Some(bytes.to_vec()); }
                Err(e) => return Json(json!({ "ok": false, "error": format!("Failed to read upload: {}", e) })),
            }
        }
    }

    let data = match file_data {
        Some(d) => d,
        None => return Json(json!({ "ok": false, "error": "No file field in multipart body" })),
    };

    // Save to upload directory
    let data_dir = crate::modules::config::get_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let upload_dir = data_dir.join("uploads");
    std::fs::create_dir_all(&upload_dir).ok();
    let save_path = upload_dir.join(&file_name);

    if let Err(e) = std::fs::write(&save_path, &data) {
        return Json(json!({ "ok": false, "error": format!("Failed to save file: {}", e) }));
    }

    let save_path_str = save_path.to_string_lossy().to_string();
    info!("[BotAPI] sendDocument(upload): file={}, size={}", file_name, data.len());

    match protocol::send_file_to_wechat(&session_id, &save_path_str).await {
        Ok(result) => {
            let file_size = data.len() as i64;
            let _ = database::save_file(&session_id, None, &file_name, &save_path_str, file_size, None);

            Json(json!({
                "ok": true,
                "result": {
                    "message_id": format!("file_{}", chrono::Utc::now().timestamp_millis()),
                    "document": { "file_name": file_name, "file_size": file_size },
                    "detail": result,
                }
            }))
        }
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

async fn send_photo(Json(body): Json<SendDocumentBody>) -> Json<Value> {
    send_document_json(Json(body)).await
}

async fn send_photo_upload(multipart: Multipart) -> Json<Value> {
    send_document_upload(multipart).await
}

async fn get_chat() -> Json<Value> {
    Json(json!({
        "ok": true,
        "result": { "id": "filehelper", "type": "private", "title": "WeChat File Transfer Assistant" }
    }))
}

/// getFile — look up file metadata by file_id (autoincrement id from files table)
async fn get_file(Query(q): Query<GetFileQuery>) -> Json<Value> {
    let file_id_str = match q.file_id {
        Some(ref id) => id.clone(),
        None => return Json(json!({ "ok": false, "error": "file_id is required" })),
    };

    let file_id: i64 = match file_id_str.parse() {
        Ok(id) => id,
        Err(_) => return Json(json!({ "ok": false, "error": "Invalid file_id" })),
    };

    match database::get_file_by_id(file_id) {
        Ok(f) => Json(json!({
            "ok": true,
            "result": {
                "file_id": f.id.to_string(),
                "file_name": f.file_name,
                "file_path": f.file_path,
                "file_size": f.file_size,
                "mime_type": f.mime_type,
            }
        })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

async fn set_webhook(Json(body): Json<SetWebhookBody>) -> Json<Value> {
    info!("[BotAPI] setWebhook: url={}", body.url);
    set_webhook_url(Some(body.url.clone()));
    Json(json!({ "ok": true, "result": true, "description": format!("Webhook set to {}", body.url) }))
}

async fn delete_webhook() -> Json<Value> {
    set_webhook_url(None);
    Json(json!({ "ok": true, "result": true, "description": "Webhook removed" }))
}

async fn get_webhook_info() -> Json<Value> {
    let url = get_webhook_url();
    Json(json!({
        "ok": true,
        "result": { "url": url.unwrap_or_default(), "has_custom_certificate": false, "pending_update_count": 0 }
    }))
}

// ============================================================================
// File management endpoints  (ported from routes/files.py)
// ============================================================================

async fn files_metadata(Query(q): Query<FilesQuery>) -> Json<Value> {
    let session_id = q.session_id.or_else(|| first_session_id().ok()).unwrap_or_default();
    if session_id.is_empty() {
        return Json(json!({ "ok": false, "error": "No active session" }));
    }

    match database::get_files(&session_id, q.limit.unwrap_or(100), q.offset.unwrap_or(0)) {
        Ok(files) => {
            let files_json: Vec<Value> = files.iter().map(|f| json!({
                "id": f.id, "file_name": f.file_name, "file_path": f.file_path,
                "file_size": f.file_size, "mime_type": f.mime_type, "created_at": f.created_at,
            })).collect();
            Json(json!({ "ok": true, "result": files_json, "count": files_json.len() }))
        }
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

async fn files_delete(Query(q): Query<GetFileQuery>) -> Json<Value> {
    let file_id_str = match q.file_id {
        Some(ref id) => id.clone(),
        None => return Json(json!({ "ok": false, "error": "file_id is required" })),
    };

    let file_id: i64 = match file_id_str.parse() {
        Ok(id) => id,
        Err(_) => return Json(json!({ "ok": false, "error": "Invalid file_id" })),
    };

    // Try to delete the actual file from disk
    if let Ok(f) = database::get_file_by_id(file_id) {
        let path = std::path::Path::new(&f.file_path);
        if path.exists() {
            std::fs::remove_file(path).ok();
        }
    }

    match database::delete_file_record(file_id) {
        Ok(_) => Json(json!({ "ok": true, "status": "deleted" })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

async fn files_cleanup(Query(q): Query<CleanupQuery>) -> Json<Value> {
    let session_id = q.session_id.or_else(|| first_session_id().ok()).unwrap_or_default();
    if session_id.is_empty() {
        return Json(json!({ "ok": false, "error": "No active session" }));
    }
    let days = q.days.unwrap_or(30);

    let deleted_msgs = database::cleanup_old_messages(&session_id, days).unwrap_or(0);
    let deleted_paths = database::cleanup_old_files(&session_id, days).unwrap_or_default();

    // Delete actual files from disk
    for path_str in &deleted_paths {
        let path = std::path::Path::new(path_str);
        if path.exists() {
            std::fs::remove_file(path).ok();
        }
    }

    info!("[BotAPI] cleanup: deleted {} messages, {} files", deleted_msgs, deleted_paths.len());

    Json(json!({
        "ok": true,
        "deleted_messages": deleted_msgs,
        "deleted_files": deleted_paths.len(),
    }))
}

async fn store_stats_handler(Query(q): Query<FilesQuery>) -> Json<Value> {
    let session_id = q.session_id.or_else(|| first_session_id().ok()).unwrap_or_default();
    if session_id.is_empty() {
        return Json(json!({ "ok": false, "error": "No active session" }));
    }

    match database::store_stats(&session_id) {
        Ok(stats) => Json(json!({ "ok": true, "result": stats })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

async fn store_messages_handler(Query(q): Query<StoreMessagesQuery>) -> Json<Value> {
    let session_id = q.session_id.or_else(|| first_session_id().ok()).unwrap_or_default();
    if session_id.is_empty() {
        return Json(json!({ "ok": false, "error": "No active session" }));
    }

    match database::get_messages(&session_id, q.limit.unwrap_or(50), q.offset.unwrap_or(0)) {
        Ok(msgs) => {
            let messages: Vec<Value> = msgs.iter().map(|m| json!({
                "id": m.id, "text": m.content, "date": m.created_at,
                "is_bot": m.ai_reply, "from_me": m.from_me, "msg_type": m.msg_type,
            })).collect();
            Json(json!({ "ok": true, "result": messages, "count": messages.len() }))
        }
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

// ============================================================================
// Convenience endpoints
// ============================================================================

async fn bot_health() -> Json<Value> {
    let has_session = first_session_id().is_ok();
    Json(json!({
        "ok": true,
        "status": if has_session { "logged_in" } else { "waiting_login" },
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn login_status(Query(q): Query<LoginStatusQuery>) -> Json<Value> {
    let session_id = q.session_id
        .or_else(|| {
            let sessions = SESSIONS.lock().unwrap();
            sessions.values().next().map(|s| s.id.clone())
        });

    match session_id {
        Some(sid) => {
            let sessions = SESSIONS.lock().unwrap();
            match sessions.get(&sid) {
                Some(s) => Json(json!({
                    "ok": true,
                    "session_id": sid,
                    "logged_in": s.session.logged_in,
                    "username": s.session.username,
                    "has_qr": !s.login_uuid.is_empty(),
                })),
                None => Json(json!({ "ok": false, "error": "Session not found" })),
            }
        }
        None => Json(json!({ "ok": false, "error": "No session exists. Call /qr first." })),
    }
}

async fn get_qr(Query(q): Query<QrQuery>) -> Json<Value> {
    let session_id = q.session_id.unwrap_or_else(ensure_session_id);

    match fh_commands::filehelper_get_qr(session_id.clone()).await {
        Ok(v) => Json(json!({
            "ok": true,
            "session_id": session_id,
            "uuid": v["uuid"],
            "qr_url": v["qr_url"],
        })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

async fn simple_send(
    Query(q): Query<SendQuery>,
    body: Option<Json<SendMessageBody>>,
) -> Json<Value> {
    let text = body.as_ref().map(|b| b.text.clone()).or(q.text.clone()).unwrap_or_default();
    if text.is_empty() {
        return Json(json!({ "ok": false, "error": "No text provided" }));
    }

    let session_id = body.as_ref().and_then(|b| b.session_id.clone())
        .or(q.session_id.clone())
        .or_else(|| first_session_id().ok())
        .unwrap_or_default();

    if session_id.is_empty() {
        return Json(json!({ "ok": false, "error": "No active session" }));
    }

    match protocol::send_text_message(&session_id, &text, false).await {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

async fn get_messages(Query(q): Query<MessagesQuery>) -> Json<Value> {
    let session_id = q.session_id.clone()
        .or_else(|| first_session_id().ok())
        .unwrap_or_default();

    if session_id.is_empty() {
        return Json(json!({ "ok": false, "error": "No active session" }));
    }

    match database::get_messages(&session_id, q.limit.unwrap_or(50), q.offset.unwrap_or(0)) {
        Ok(msgs) => {
            let messages: Vec<Value> = msgs.iter().map(|m| {
                json!({
                    "message_id": m.id, "text": m.content, "date": m.created_at,
                    "is_bot": m.ai_reply, "from_me": m.from_me, "msg_type": m.msg_type,
                })
            }).collect();
            Json(json!({ "ok": true, "result": messages, "count": messages.len() }))
        }
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

// ============================================================================
// Heartbeat monitor — auto-reconnect when session drops
// ============================================================================

/// Start background heartbeat monitor that checks session health
/// and marks sessions offline after consecutive failures.
pub fn start_heartbeat_monitor() {
    tauri::async_runtime::spawn(async move {
        let check_interval = tokio::time::Duration::from_secs(60);
        let mut interval = tokio::time::interval(check_interval);
        let mut consecutive_failures: u32 = 0;

        loop {
            interval.tick().await;

            let session_ids: Vec<String> = {
                let sessions = SESSIONS.lock().unwrap();
                sessions.values()
                    .filter(|s| s.session.logged_in)
                    .map(|s| s.id.clone())
                    .collect()
            };

            for sid in session_ids {
                match protocol::sync_check(&sid).await {
                    Ok(_) => {
                        consecutive_failures = 0;
                    }
                    Err(e) => {
                        consecutive_failures += 1;
                        warn!("[Heartbeat] session {} check failed (attempt {}): {}", sid, consecutive_failures, e);

                        if consecutive_failures >= 3 {
                            error!("[Heartbeat] session {} appears dead after {} failures, marking offline", sid, consecutive_failures);
                            let mut sessions = SESSIONS.lock().unwrap();
                            if let Some(s) = sessions.get_mut(&sid) {
                                s.session.logged_in = false;
                            }
                            consecutive_failures = 0;
                        }
                    }
                }
            }
        }
    });
}

// ============================================================================
// copyMessage — re-send existing message by ID
// ============================================================================

#[derive(Deserialize)]
struct CopyMessageBody {
    message_id: i64,
    #[serde(default)]
    session_id: Option<String>,
}

/// copyMessage — look up a message by ID and re-send its content through WeChat.
async fn copy_message(Json(body): Json<CopyMessageBody>) -> Json<Value> {
    let session_id = body.session_id
        .or_else(|| first_session_id().ok())
        .unwrap_or_default();

    if session_id.is_empty() {
        return Json(json!({ "ok": false, "error": "No active session" }));
    }

    // Look up original message
    let msgs = match database::get_messages(&session_id, 1000, 0) {
        Ok(m) => m,
        Err(e) => return Json(json!({ "ok": false, "error": e })),
    };

    let original = match msgs.iter().find(|m| m.id == body.message_id) {
        Some(m) => m,
        None => return Json(json!({ "ok": false, "error": "Message not found" })),
    };

    if original.content.is_empty() {
        return Json(json!({ "ok": false, "error": "Message has no content" }));
    }

    info!("[BotAPI] copyMessage: re-sending message_id={}", body.message_id);

    match protocol::send_text_message(&session_id, &original.content, false).await {
        Ok(_) => {
            let new_id = database::save_message(&session_id, &original.content, true, 1, false).unwrap_or(0);
            Json(json!({
                "ok": true,
                "result": { "message_id": new_id }
            }))
        }
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

// ============================================================================
// WeChat extension endpoints  (ported from routes/wechat.py)
// ============================================================================

/// Save session to disk (persist login for later restore)
async fn wechat_session_save() -> Json<Value> {
    super::save_sessions_to_disk();
    info!("[BotAPI] Session saved to disk");
    Json(json!({ "ok": true }))
}

// ============================================================================
// Router builder
// ============================================================================

pub fn bot_api_routes() -> Router {
    Router::new()
        // TG Bot API standard endpoints
        .route("/bot/getMe", get(get_me))
        .route("/bot/getUpdates", get(get_updates))
        .route("/bot/sendMessage", post(send_message))
        .route("/bot/sendDocument", post(send_document_json))
        .route("/bot/sendDocument/upload", post(send_document_upload))
        .route("/bot/sendPhoto", post(send_photo))
        .route("/bot/sendPhoto/upload", post(send_photo_upload))
        .route("/bot/getChat", get(get_chat))
        .route("/bot/getFile", get(get_file))
        .route("/bot/copyMessage", post(copy_message))
        .route("/bot/setWebhook", post(set_webhook))
        .route("/bot/deleteWebhook", post(delete_webhook))
        .route("/bot/getWebhookInfo", get(get_webhook_info))
        // File management endpoints
        .route("/files/metadata", get(files_metadata))
        .route("/files/delete", delete(files_delete))
        .route("/files/cleanup", post(files_cleanup))
        // Store endpoints
        .route("/store/stats", get(store_stats_handler))
        .route("/store/messages", get(store_messages_handler))
        // WeChat extension endpoints
        .route("/wechat/session/save", post(wechat_session_save))
        // Convenience endpoints
        .route("/health", get(bot_health))
        .route("/login/status", get(login_status))
        .route("/qr", get(get_qr))
        .route("/send", post(simple_send))
        .route("/messages", get(get_messages))
}

