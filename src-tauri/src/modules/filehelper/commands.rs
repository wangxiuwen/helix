//! Tauri commands for the WeChat File Transfer Assistant frontend.

use serde_json::{json, Value};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info, warn, error};

use crate::modules::database;
use super::{
    SESSIONS, save_sessions_to_disk,
    build_client, build_no_redirect_client,
    FileHelperSession,
};
use super::protocol;

// ============================================================================
// Tauri Commands
// ============================================================================

/// Create a new session and return its ID
#[tauri::command]
pub async fn filehelper_create_session() -> Result<Value, String> {
    let session_id = uuid::Uuid::new_v4().to_string();

    {
        let mut sessions = SESSIONS.lock().unwrap();
        sessions.insert(session_id.clone(), super::SessionState {
            id: session_id.clone(),
            session: FileHelperSession::default(),
            login_uuid: String::new(),
            client: build_client(),
            no_redirect_client: build_no_redirect_client(),
            is_polling: Arc::new(AtomicBool::new(false)),
            last_msg_count: 0,
        });
    }

    info!("Created new session: {}", session_id);
    Ok(json!({ "session_id": session_id }))
}

/// List all active sessions
#[tauri::command]
pub async fn filehelper_list_sessions() -> Result<Value, String> {
    let mut sessions = SESSIONS.lock().unwrap();
    
    // Cleanup: remove any session that isn't logged in
    sessions.retain(|_, s| s.session.logged_in);

    let list: Vec<Value> = sessions.values().map(|s| {
        json!({
            "id": s.id,
            "logged_in": s.session.logged_in,
            "username": s.session.username,
        })
    }).collect();
    Ok(json!({ "sessions": list }))
}

/// Get QR code URL for a session
#[tauri::command]
pub async fn filehelper_get_qr(session_id: String) -> Result<Value, String> {
    let uuid = protocol::generate_qr_uuid(&session_id).await?;
    let qr_url = protocol::get_qr_url(&uuid);
    Ok(json!({
        "uuid": uuid,
        "qr_url": qr_url
    }))
}

/// Poll login status for a session
#[tauri::command]
pub async fn filehelper_check_login(session_id: String) -> Result<Value, String> {
    let uuid = {
        let sessions = SESSIONS.lock().unwrap();
        let s = sessions.get(&session_id)
            .ok_or_else(|| format!("Session '{}' not found", session_id))?;
        s.login_uuid.clone()
    };

    if uuid.is_empty() {
        return Err("No QR code generated yet".to_string());
    }

    let status = protocol::check_login_status(&session_id, &uuid).await;
    match status {
        Ok(s) => {
            if s == "logged_in" {
                let nickname = match protocol::webwx_init(&session_id).await {
                    Ok(n) => n,
                    Err(e) => {
                        error!("[{}] webwx_init failed: {}", session_id, e);
                        return Err(e);
                    }
                };
                Ok(json!({
                    "status": "logged_in",
                    "nickname": nickname
                }))
            } else {
                Ok(json!({ "status": s }))
            }
        }
        Err(e) if e == "waiting" => Ok(json!({ "status": "waiting" })),
        Err(e) => {
            error!("[{}] check_login_status failed: {}", session_id, e);
            Err(e)
        }
    }
}

/// Get status of a session
#[tauri::command]
pub async fn filehelper_status(session_id: String) -> Result<Value, String> {
    let sessions = SESSIONS.lock().unwrap();
    let s = sessions.get(&session_id)
        .ok_or_else(|| format!("Session '{}' not found", session_id))?;
    Ok(json!({
        "logged_in": s.session.logged_in,
        "username": s.session.username,
    }))
}

/// Send a text message for a session
#[tauri::command]
pub async fn filehelper_send_msg(session_id: String, content: String) -> Result<Value, String> {
    protocol::send_text_message(&session_id, &content, false).await?;

    // Spawn AI auto-reply in background
    let sid = session_id.clone();
    let user_content = content.clone();
    tauri::async_runtime::spawn(async move {
        let ack_msg = "🫡 收到，正在处理...";
        let _ = protocol::send_text_message(&sid, ack_msg, true).await;

        match crate::modules::agent::agent_process_message(&sid, &user_content).await {
            Ok(reply) => {
                info!("[{}] Agent reply sent from UI message", sid);
                let _ = protocol::send_text_message(&sid, &reply, true).await;
            }
            Err(e) => {
                warn!("[{}] Agent auto-reply error: {}", sid, e);
                let err_msg = format!("❌ 执行出错: {}", e);
                let _ = protocol::send_text_message(&sid, &err_msg, true).await;
            }
        }
    });

    Ok(json!({ "ok": true }))
}

/// Send a file through the File Transfer Assistant channel
#[tauri::command]
pub async fn filehelper_send_file(session_id: String, file_path: String) -> Result<Value, String> {
    let result = protocol::send_file_to_wechat(&session_id, &file_path).await?;
    Ok(json!({ "ok": true, "message": result }))
}

/// Core polling logic — used by both the Tauri command and the background poller.
pub(crate) async fn poll_messages_inner(session_id: &str) -> Result<Value, String> {
    let is_polling = {
        let sessions = SESSIONS.lock().unwrap();
        if let Some(s) = sessions.get(session_id) {
            s.is_polling.clone()
        } else {
            return Err("Session not found".to_string());
        }
    };

    if is_polling.swap(true, Ordering::SeqCst) {
        return Ok(json!({ "has_new": false, "messages": [] }));
    }

    let result = async {
        let has_msg = match protocol::sync_check(session_id).await {
            Ok(v) => v,
            Err(e) => {
                if e.starts_with("expired:") {
                    info!("[{}] Session expired detected in sync_check: {}", session_id, e);
                    {
                        let mut sessions = SESSIONS.lock().unwrap();
                        if let Some(s) = sessions.get_mut(session_id) {
                            s.session.logged_in = false;
                        }
                    }
                    save_sessions_to_disk();
                    return Ok(json!({
                        "has_new": false,
                        "messages": [],
                        "expired": true,
                        "error": e
                    }));
                }
                return Err(e);
            }
        };

        let mut db_changed = false;
        let db_count = database::count_messages(session_id).unwrap_or(0);
        {
            let mut sessions = SESSIONS.lock().unwrap();
            if let Some(s) = sessions.get_mut(session_id) {
                if s.last_msg_count != db_count {
                    s.last_msg_count = db_count;
                    db_changed = true;
                }
            }
        }

        if has_msg || db_changed {
            let mut msgs = Vec::new();
            if has_msg {
                msgs = match protocol::receive_messages(session_id).await {
                    Ok(m) => m,
                    Err(e) => {
                        if e.starts_with("expired:") {
                            info!("[{}] Session expired detected in receive_messages: {}", session_id, e);
                            {
                                let mut sessions = SESSIONS.lock().unwrap();
                                if let Some(s) = sessions.get_mut(session_id) {
                                    s.session.logged_in = false;
                                }
                            }
                            save_sessions_to_disk();
                            return Ok(json!({
                                "has_new": false,
                                "messages": [],
                                "expired": true,
                                "error": e
                            }));
                        }
                        return Err(e);
                    }
                };
            }

            let account_auto_reply = true;
            if account_auto_reply && has_msg {
                for msg in msgs.clone() {
                    if !msg.is_bot && msg.msg_type == 1 && !msg.content.is_empty() {
                        // Push received message to webhook
                        super::bot_api::push_to_webhook(json!({
                            "update_id": msg.timestamp,
                            "message": {
                                "text": &msg.content,
                                "date": msg.timestamp,
                                "from": { "id": "user", "is_bot": false },
                                "chat": { "id": "filehelper", "type": "private" },
                            }
                        }));

                        let sid = session_id.to_string();
                        let content = msg.content.clone();
                        
                        tauri::async_runtime::spawn(async move {
                            // Try bot commands first (e.g. /start, /help, /about)
                            if let Some(reply) = super::bot_commands::dispatch_command(&content, &sid) {
                                let _ = protocol::send_text_message(&sid, &reply, true).await;
                                return;
                            }

                            // Not a command — use AI agent
                            let ack_msg = "🫡 收到，正在处理...";
                            let _ = protocol::send_text_message(&sid, ack_msg, true).await;

                            match crate::modules::agent::agent_process_message(&sid, &content).await {
                                Ok(reply) => {
                                    info!("[{}] Agent reply sent asynchronously", sid);
                                    let _ = protocol::send_text_message(&sid, &reply, true).await;
                                }
                                Err(e) => {
                                    warn!("[{}] Agent auto-reply error: {}", sid, e);
                                    let err_msg = format!("❌ 执行出错: {}", e);
                                    let _ = protocol::send_text_message(&sid, &err_msg, true).await;
                                }
                            }
                        });
                    }
                }
            }

            Ok(json!({
                "has_new": true,
                "messages": msgs
            }))
        } else {
            Ok(json!({ "has_new": false, "messages": [] }))
        }
    }.await;

    is_polling.store(false, Ordering::SeqCst);
    result
}

/// Poll for new messages — with optional AI auto-reply
#[tauri::command]
pub async fn filehelper_poll_messages(session_id: String) -> Result<Value, String> {
    poll_messages_inner(&session_id).await
}

/// Get messages from database
#[tauri::command]
pub async fn filehelper_get_messages(session_id: String, limit: Option<i64>, offset: Option<i64>) -> Result<Value, String> {
    let messages = database::get_messages(&session_id, limit.unwrap_or(100), offset.unwrap_or(0))?;
    Ok(json!({ "messages": messages }))
}

/// Logout a session
#[tauri::command]
pub async fn filehelper_logout(session_id: String) -> Result<Value, String> {
    {
        let mut sessions = SESSIONS.lock().unwrap();
        sessions.remove(&session_id);
    }
    save_sessions_to_disk();
    info!("[{}] Session logged out", session_id);
    Ok(json!({ "ok": true }))
}
