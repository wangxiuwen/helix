//! WeChat File Transfer Assistant (文件传输助手) web protocol integration.
//!
//! Multi-session support: each WeChat account gets its own session with
//! independent login, messaging, and cookie store.

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::redirect;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use once_cell::sync::Lazy;
use regex::Regex;
use tracing::{info, warn, error};

use super::database;

// ============================================================================
// Constants
// ============================================================================

const WX_LOGIN_HOST: &str = "https://login.wx.qq.com";
const WX_FILEHELPER_HOST: &str = "https://szfilehelper.weixin.qq.com";
const APP_ID: &str = "wx_webfilehelper";

// ============================================================================
// Data structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileHelperSession {
    pub uin: String,
    pub sid: String,
    pub skey: String,
    pub pass_ticket: String,
    pub webwx_data_ticket: String,
    pub username: String,
    pub username_hash: String,
    pub sync_key: Option<Value>,
    pub logged_in: bool,
    pub raw_cookies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub content: String,
    pub from_me: bool,
    pub is_bot: bool,
    pub timestamp: u64,
    pub msg_type: i32,
}

// ============================================================================
// Multi-session state
// ============================================================================

struct SessionState {
    id: String,
    session: FileHelperSession,
    login_uuid: String,
    client: reqwest::Client,           // per-session with own cookie store
    no_redirect_client: reqwest::Client,
    is_polling: Arc<AtomicBool>,
    last_msg_count: i64,
}

fn get_sessions_path() -> std::path::PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push(".helix");
    let _ = std::fs::create_dir_all(&path);
    path.push("wechat_sessions.json");
    path
}

fn load_sessions_from_disk() -> HashMap<String, SessionState> {
    let mut result = HashMap::new();
    if let Ok(json) = std::fs::read_to_string(get_sessions_path()) {
        if let Ok(saved) = serde_json::from_str::<HashMap<String, FileHelperSession>>(&json) {
            for (id, mut session) in saved {
                // Restore webwx_data_ticket from cookies if not already set
                if session.webwx_data_ticket.is_empty() {
                    if let Some(ticket) = session.raw_cookies.iter().find_map(|c| {
                        c.split(';').next().and_then(|part| {
                            let part = part.trim();
                            if part.starts_with("webwx_data_ticket=") {
                                Some(part["webwx_data_ticket=".len()..].to_string())
                            } else {
                                None
                            }
                        })
                    }) {
                        session.webwx_data_ticket = ticket;
                    }
                }

                let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
                if let Ok(url1) = "https://wx.qq.com".parse::<reqwest::Url>() {
                    for c in &session.raw_cookies { jar.add_cookie_str(c, &url1); }
                }
                if let Ok(url2) = "https://szfilehelper.weixin.qq.com".parse::<reqwest::Url>() {
                    for c in &session.raw_cookies { jar.add_cookie_str(c, &url2); }
                }
                if let Ok(url3) = "https://login.wx.qq.com".parse::<reqwest::Url>() {
                    for c in &session.raw_cookies { jar.add_cookie_str(c, &url3); }
                }
                if let Ok(url4) = "https://file.wx2.qq.com".parse::<reqwest::Url>() {
                    for c in &session.raw_cookies { jar.add_cookie_str(c, &url4); }
                }
                
                let client = reqwest::Client::builder()
                    .cookie_provider(jar)
                    .danger_accept_invalid_certs(true)
                    .http1_only()  // WeChat file.wx2.qq.com doesn't support HTTP/2 multipart uploads properly
                    .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                    .default_headers(default_headers())
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());
                    
                result.insert(id.clone(), SessionState {
                    id: id.clone(),
                    session,
                    login_uuid: String::new(),
                    client,
                    no_redirect_client: build_no_redirect_client(),
                    is_polling: Arc::new(AtomicBool::new(false)),
                    last_msg_count: 0,
                });
            }
        }
    }
    info!("Loaded {} restored wechat sessions from disk.", result.len());
    result
}

fn save_sessions_to_disk() {
    let sessions = SESSIONS.lock().unwrap();
    let mut to_save = HashMap::new();
    for (id, s) in sessions.iter() {
        if s.session.logged_in {
            to_save.insert(id.clone(), s.session.clone());
        }
    }
    if let Ok(json) = serde_json::to_string(&to_save) {
        let _ = std::fs::write(get_sessions_path(), json);
    }
}

static SESSIONS: Lazy<Mutex<HashMap<String, SessionState>>> = Lazy::new(|| {
    Mutex::new(load_sessions_from_disk())
});

/// Start a background tokio task that polls WeChat messages every 3 seconds,
/// independent of whether the frontend WeChat page is mounted.
pub fn start_background_polling() {
    // Force SESSIONS lazy init so sessions are loaded from disk
    {
        let sessions = SESSIONS.lock().unwrap();
        info!("Background polling: found {} sessions", sessions.len());
    }

    tauri::async_runtime::spawn(async {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            // Collect logged-in session IDs
            let session_ids: Vec<String> = {
                let sessions = SESSIONS.lock().unwrap();
                sessions.iter()
                    .filter(|(_, s)| s.session.logged_in)
                    .map(|(id, _)| id.clone())
                    .collect()
            };

            for sid in session_ids {
                if let Err(e) = filehelper_poll_messages_inner(&sid).await {
                    // Only log non-trivial errors
                    if !e.contains("Session not found") {
                        warn!("[bg-poll] Error polling {}: {}", &sid[..8.min(sid.len())], e);
                    }
                }
            }
        }
    });
}

/// Plaintext prefix added to all bot-sent messages.
/// Used to distinguish bot replies from user messages in file helper (self-chat).
const BOT_PREFIX: &str = "[Helix] ";

// ============================================================================
// Helpers
// ============================================================================

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn generate_device_id() -> String {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}", seed % 1_000_000_000_000_000u128)
}

fn generate_msg_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = ts.as_secs();
    let nanos = ts.subsec_nanos();
    format!("{}{}{}", secs, nanos / 1000, nanos % 10)
}

fn default_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("accept", HeaderValue::from_static("*/*"));
    h.insert("accept-language", HeaderValue::from_static("zh,zh-CN;q=0.9,en;q=0.8"));
    h.insert("cache-control", HeaderValue::from_static("no-cache"));
    h.insert("mmweb_appid", HeaderValue::from_static(APP_ID));
    h
}

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .danger_accept_invalid_certs(true)
        .http1_only()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .default_headers(default_headers())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn build_no_redirect_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .danger_accept_invalid_certs(true)
        .http1_only()
        .redirect(redirect::Policy::none())
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .default_headers(default_headers())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn regex_match(pattern: &str, text: &str) -> Result<String, String> {
    let re = Regex::new(pattern).map_err(|e| format!("Regex error: {}", e))?;
    re.captures(text)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| format!("Pattern not found: {} in text: {}", pattern, &text[..text.len().min(200)]))
}

/// Get a snapshot of session data needed for API calls (avoids holding lock during async)
fn get_session_snapshot(session_id: &str) -> Result<(FileHelperSession, reqwest::Client), String> {
    let sessions = SESSIONS.lock().unwrap();
    let s = sessions.get(session_id)
        .ok_or_else(|| format!("Session '{}' not found", session_id))?;
    Ok((s.session.clone(), s.client.clone()))
}

fn get_no_redirect_client(session_id: &str) -> Result<reqwest::Client, String> {
    let sessions = SESSIONS.lock().unwrap();
    let s = sessions.get(session_id)
        .ok_or_else(|| format!("Session '{}' not found", session_id))?;
    Ok(s.no_redirect_client.clone())
}

// ============================================================================
// Login flow
// ============================================================================

async fn generate_qr_uuid(session_id: &str) -> Result<String, String> {
    let params = [
        ("appid", APP_ID),
        ("fun", "new"),
        ("lang", "zh_CN"),
    ];

    let (_, client) = get_session_snapshot(session_id)?;

    let resp = client
        .get(&format!("{}/jslogin", WX_LOGIN_HOST))
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("jslogin request failed: {}", e))?;

    let text = resp.text().await.map_err(|e| format!("Read body failed: {}", e))?;
    info!("[{}] jslogin response: {}", session_id, &text[..text.len().min(200)]);
    let uuid = regex_match(r#"window\.QRLogin\.uuid = "(.*?)";"#, &text)?;

    {
        let mut sessions = SESSIONS.lock().unwrap();
        if let Some(s) = sessions.get_mut(session_id) {
            s.login_uuid = uuid.clone();
        }
    }

    info!("[{}] Generated QR UUID: {}", session_id, uuid);
    Ok(uuid)
}

fn get_qr_url(uuid: &str) -> String {
    format!("{}/qrcode/{}", WX_LOGIN_HOST, uuid)
}

async fn check_login_status(session_id: &str, uuid: &str) -> Result<String, String> {
    let (_, client) = get_session_snapshot(session_id)?;

    let params = [
        ("loginicon", "true"),
        ("uuid", uuid),
        ("tip", "1"),
        ("appid", APP_ID),
    ];

    let resp = client
        .get(&format!("{}/cgi-bin/mmwebwx-bin/login", WX_LOGIN_HOST))
        .query(&params)
        .timeout(std::time::Duration::from_secs(25))
        .send()
        .await
        .map_err(|e| format!("Login check failed: {}", e))?;

    let text = resp.text().await.map_err(|e| format!("Read body failed: {}", e))?;
    let code = regex_match(r#"window\.code\s*=\s*(\d+);"#, &text)?;

    match code.as_str() {
        "408" => Err("waiting".to_string()),
        "201" => Ok("scanned".to_string()),
        "200" => {
            let redirect_url = regex_match(r#"window\.redirect_uri="(.*?)";"#, &text)?;
            let redirect_url = redirect_url.replacen('?', "?fun=new&version=v2&", 1);

            info!("[{}] Login success! Redirect URL: {}", session_id, &redirect_url[..redirect_url.len().min(100)]);

            let no_redirect_client = get_no_redirect_client(session_id)?;
            let resp = no_redirect_client
                .get(&redirect_url)
                .send()
                .await
                .map_err(|e| format!("Redirect failed: {}", e))?;

            let mut captured_cookies = Vec::new();
            for header in resp.headers().get_all(reqwest::header::SET_COOKIE) {
                if let Ok(c_str) = header.to_str() {
                    captured_cookies.push(c_str.to_string());
                }
            }

            let body = resp.text().await.map_err(|e| format!("Read redirect body: {}", e))?;
            info!("[{}] Login redirect body (first 300 chars): {}", session_id, &body[..body.len().min(300)]);

            let skey = regex_match(r"<skey>(.*?)</skey>", &body).unwrap_or_default();
            let wxsid = regex_match(r"<wxsid>(.*?)</wxsid>", &body).unwrap_or_default();
            let wxuin = regex_match(r"<wxuin>(.*?)</wxuin>", &body).unwrap_or_default();
            let pass_ticket = regex_match(r"<pass_ticket>(.*?)</pass_ticket>", &body).unwrap_or_default();
            
            // If all of these are empty, we probably hit a redirect error page instead of the XML response
            if skey.is_empty() && wxsid.is_empty() && wxuin.is_empty() {
                // Parse WeChat error codes for user-friendly messages
                let ret_code = regex_match(r"<ret>(.*?)</ret>", &body).unwrap_or_default();
                let wechat_msg = regex_match(r"<message>(.*?)</message>", &body).unwrap_or_default();
                
                let friendly_msg = match ret_code.as_str() {
                    "1203" => "登录失败：此微信号不能登录网页版微信。请尝试使用其他微信号，或在手机微信「设置 → 账号与安全」中检查网页登录权限。".to_string(),
                    "1100" => "登录失败：微信已在其他地方登录网页版".to_string(),
                    "1101" => "登录失败：会话已过期，请重新扫码".to_string(),
                    "1102" => "登录失败：操作频率过快，请稍后再试".to_string(),
                    code if !code.is_empty() => {
                        let extra = if wechat_msg.is_empty() { String::new() } else { format!(" ({})", wechat_msg) };
                        format!("登录失败：微信返回错误码 {}{}", code, extra)
                    },
                    _ => format!("登录失败：无法解析登录凭据，响应内容: {}", &body[..body.len().min(200)]),
                };
                
                return Err(friendly_msg);
            }

            {
                let mut sessions = SESSIONS.lock().unwrap();
                if let Some(s) = sessions.get_mut(session_id) {
                    s.session.skey = skey;
                    s.session.sid = wxsid;
                    s.session.uin = wxuin;
                    s.session.pass_ticket = pass_ticket;
                    if !captured_cookies.is_empty() {
                        s.session.raw_cookies = captured_cookies;
                    }
                }
            }

            Ok("logged_in".to_string())
        }
        "400" | "500" => Err(format!("Server returned error code {}", code)),
        "" => Err("Empty response code parsing window.code".to_string()),
        other => Err(format!("Unknown login code: {}", other)),
    }
}

async fn webwx_init(session_id: &str) -> Result<String, String> {
    let (session, client) = get_session_snapshot(session_id)?;

    let base_request = json!({
        "Uin": session.uin,
        "Sid": session.sid,
        "Skey": session.skey,
        "DeviceID": generate_device_id()
    });

    let params = [("lang", "zh_CN"), ("pass_ticket", session.pass_ticket.as_str())];
    let body = json!({ "BaseRequest": base_request });

    let resp = client
        .post(&format!("{}/cgi-bin/mmwebwx-bin/webwxinit", WX_FILEHELPER_HOST))
        .query(&params)
        .header(CONTENT_TYPE, "application/json")
        .header("mmweb_appid", APP_ID)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("webwxinit failed: {}", e))?;

    let mut init_cookies = Vec::new();
    for header in resp.headers().get_all(reqwest::header::SET_COOKIE) {
        if let Ok(c_str) = header.to_str() {
            init_cookies.push(c_str.to_string());
        }
    }

    let bytes = resp.bytes().await.map_err(|e| format!("Read init body: {}", e))?;
    let text = String::from_utf8_lossy(&bytes);
    let data: Value = serde_json::from_str(&text)
        .map_err(|e| format!("Parse init JSON: {} — body: {}", e, &text[..text.len().min(500)]))?;

    let ret = data["BaseResponse"]["Ret"].as_i64().unwrap_or(-1);
    info!("[{}] webwxinit Ret={}", session_id, ret);
    if ret != 0 {
        let err_msg = data["BaseResponse"]["ErrMsg"].as_str().unwrap_or("unknown");
        return Err(format!("webwxinit returned Ret={}, ErrMsg={}", ret, err_msg));
    }

    let nickname = data["User"]["NickName"]
        .as_str()
        .unwrap_or("微信用户")
        .to_string();
    let username_hash = data["User"]["UserName"]
        .as_str()
        .unwrap_or("")
        .to_string();

    {
        let mut sessions = SESSIONS.lock().unwrap();
        if let Some(s) = sessions.get_mut(session_id) {
            s.session.username = nickname.clone();
            s.session.username_hash = username_hash;
            s.session.sync_key = Some(data["SyncKey"].clone());
            s.session.logged_in = true;
            if !init_cookies.is_empty() {
                s.session.raw_cookies.extend(init_cookies);
            }
            // Extract webwx_data_ticket from cookies (needed for file upload)
            let ticket = s.session.raw_cookies.iter()
                .find_map(|c| {
                    c.split(';').next()
                        .and_then(|part| {
                            let part = part.trim();
                            if part.starts_with("webwx_data_ticket=") {
                                Some(part["webwx_data_ticket=".len()..].to_string())
                            } else {
                                None
                            }
                        })
                })
                .unwrap_or_default();
            if !ticket.is_empty() {
                info!("[{}] Extracted webwx_data_ticket (len={})", session_id, ticket.len());
                s.session.webwx_data_ticket = ticket;
            } else {
                warn!("[{}] webwx_data_ticket not found in cookies — file upload may fail", session_id);
            }
        }
    }

    // Save account to database
    if let Err(e) = database::create_account(session_id, &nickname) {
        warn!("[{}] Failed to save account to DB: {}", session_id, e);
    }

    save_sessions_to_disk();

    Ok(nickname)
}

// ============================================================================
// Messaging
// ============================================================================

fn build_base_request(session: &FileHelperSession) -> Result<Value, String> {
    if !session.logged_in {
        return Err("Not logged in".to_string());
    }
    Ok(json!({
        "Uin": session.uin,
        "Sid": session.sid,
        "Skey": session.skey,
        "DeviceID": generate_device_id()
    }))
}

pub async fn send_text_message(session_id: &str, content: &str, is_bot: bool) -> Result<(), String> {
    let (session, client) = get_session_snapshot(session_id)?;
    let base_request = build_base_request(&session)?;
    let msg_id = generate_msg_id();

    // Add bot prefix so we can identify our own messages in self-chat
    let tagged_content = if is_bot {
        format!("{}{}", BOT_PREFIX, content)
    } else {
        content.to_string()
    };

    let body = json!({
        "BaseRequest": base_request,
        "Msg": {
            "ClientMsgId": msg_id,
            "FromUserName": session.username_hash,
            "LocalID": msg_id,
            "ToUserName": "filehelper",
            "Content": tagged_content,
            "Type": 1,
            "MediaId": ""
        },
        "Scene": 0
    });

    let params = [("lang", "zh_CN"), ("pass_ticket", session.pass_ticket.as_str())];

    let resp = client
        .post(&format!("{}/cgi-bin/mmwebwx-bin/webwxsendmsg", WX_FILEHELPER_HOST))
        .query(&params)
        .header(CONTENT_TYPE, "application/json")
        .header("mmweb_appid", APP_ID)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Send msg failed: {}", e))?;

    let data: Value = resp.json().await.map_err(|e| format!("Parse send response: {}", e))?;
    let ret = data["BaseResponse"]["Ret"].as_i64().unwrap_or(-1);

    if ret != 0 {
        return Err(format!("Send message failed, Ret={}", ret));
    }

    // Save to database immediately so it shows up in history and frontend polling
    let _ = database::save_message(session_id, &content, true, 1, is_bot);

    Ok(())
}

/// Upload a file to WeChat servers and send it through the File Transfer Assistant channel.
/// Reference: https://github.com/zzzzls/WXFileHelper
pub async fn send_file_to_wechat(session_id: &str, file_path: &str) -> Result<String, String> {
    const WX_FILEUPLOAD_HOST: &str = "https://file.wx2.qq.com";

    let path = std::path::Path::new(file_path);
    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    let file_name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let file_ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();
    let file_bytes = std::fs::read(path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    let file_size = file_bytes.len();

    // Compute MD5 via macOS command (avoid adding crate dependency)
    let md5_output = std::process::Command::new("md5")
        .arg("-q").arg(file_path)
        .output()
        .map_err(|e| format!("md5 command failed: {}", e))?;
    let file_md5 = String::from_utf8_lossy(&md5_output.stdout).trim().to_string();

    // Determine file category - WeChat Web API only distinguishes image vs doc
    let is_image = matches!(file_ext.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp");
    // All non-image files (including video, audio, docs) use mediatype='doc'
    let media_type_str = if is_image { "pic" } else { "doc" };

    let mime_type = match file_ext.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "txt" | "log" | "md" => "text/plain",
        "json" => "application/json",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    };

    let (session, client) = get_session_snapshot(session_id)?;
    let base_request = build_base_request(&session)?;
    let client_media_id = generate_msg_id();

    // Step 1: Upload the file via multipart to file.wx2.qq.com
    let upload_media_request = json!({
        "UploadType": 2,
        "BaseRequest": base_request,
        "ClientMediaId": client_media_id,
        "TotalLen": file_size,
        "StartPos": 0,
        "DataLen": file_size,
        "MediaType": 4,
        "FromUserName": session.username_hash,
        "ToUserName": "filehelper",
        "FileMd5": file_md5
    });

    let file_part = reqwest::multipart::Part::bytes(file_bytes)
        .file_name(file_name.clone())
        .mime_str(mime_type)
        .map_err(|e| format!("Multipart error: {}", e))?;

    let form = reqwest::multipart::Form::new()
        .text("id", "WU_FILE_0")
        .text("name", file_name.clone())
        .text("type", mime_type.to_string())
        .text("lastModifiedDate", "Thu Jan 01 1970 08:00:00 GMT+0800")
        .text("size", file_size.to_string())
        .text("mediatype", media_type_str.to_string())
        .text("uploadmediarequest", upload_media_request.to_string())
        .text("webwx_data_ticket", session.webwx_data_ticket.clone())
        .text("pass_ticket", session.pass_ticket.clone())
        .part("filename", file_part);

    let upload_url = format!("{}/cgi-bin/mmwebwx-bin/webwxuploadmedia", WX_FILEUPLOAD_HOST);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos();
    let random_key = format!("{:04x}", nanos % 0xFFFF);

    info!("[{}] Uploading file '{}' ({} bytes, md5={}, mediatype={}, ticket_len={}) to {}",
        session_id, file_name, file_size, &file_md5[..8], media_type_str,
        session.webwx_data_ticket.len(), WX_FILEUPLOAD_HOST);

    let upload_resp = client
        .post(&upload_url)
        .query(&[("f", "json"), ("random", &random_key)])
        .header("mmweb_appid", APP_ID)
        .header("Origin", WX_FILEHELPER_HOST)
        .header("Referer", WX_FILEHELPER_HOST)
        .multipart(form)
        .send()
        .await
        .map_err(|e| {
            let err_msg = format!("Upload failed: {:?}", e);
            let _ = std::fs::write("/tmp/wechat_upload_error.txt", &err_msg);
            err_msg
        })?;

    let upload_status = upload_resp.status();
    let upload_text = upload_resp.text().await
        .map_err(|e| format!("Read upload response: {}", e))?;

    info!("[{}] Upload response (status={}): {}", session_id, upload_status, &upload_text);

    let upload_data: Value = serde_json::from_str(&upload_text)
        .map_err(|e| format!("Parse upload response: {} — body: {}", e,
            crate::utils::truncate::safe_truncate(&upload_text, 200)))?;

    let upload_ret = upload_data["BaseResponse"]["Ret"].as_i64().unwrap_or(-1);
    if upload_ret != 0 {
        return Err(format!("Upload failed, Ret={}, response: {}",
            upload_ret, crate::utils::truncate::safe_truncate(&upload_text, 200)));
    }

    let media_id = upload_data["MediaId"].as_str()
        .ok_or_else(|| format!("No MediaId in upload response: {}",
            crate::utils::truncate::safe_truncate(&upload_text, 200)))?
        .to_string();

    info!("[{}] File uploaded, MediaId={}", session_id,
        crate::utils::truncate::safe_truncate(&media_id, 30));

    // Step 2: Send the message
    let msg_id = generate_msg_id();
    let params = [("lang", "zh_CN"), ("pass_ticket", session.pass_ticket.as_str())];

    if is_image {
        // Images: use webwxsendmsgimg with Type=3
        let send_body = json!({
            "BaseRequest": base_request,
            "Msg": {
                "Type": 3,
                "Content": "",
                "FromUserName": session.username_hash,
                "ToUserName": "filehelper",
                "LocalID": msg_id,
                "ClientMsgId": msg_id,
                "MediaId": media_id
            },
            "Scene": 0
        });

        let send_resp = client
            .post(&format!("{}/cgi-bin/mmwebwx-bin/webwxsendmsgimg", WX_FILEHELPER_HOST))
            .query(&[("fun", "async"), ("f", "json"), ("lang", "zh_CN"), ("pass_ticket", session.pass_ticket.as_str())])
            .header(CONTENT_TYPE, "application/json")
            .header("mmweb_appid", APP_ID)
            .json(&send_body)
            .send()
            .await
            .map_err(|e| format!("Send image msg failed: {}", e))?;

        let send_data: Value = send_resp.json().await
            .map_err(|e| format!("Parse send response: {}", e))?;

        let ret = send_data["BaseResponse"]["Ret"].as_i64().unwrap_or(-1);
        if ret != 0 {
            return Err(format!("Send image message failed, Ret={}", ret));
        }
    } else {
        // Files (including video, audio, docs): use webwxsendappmsg with Type=6
        let app_msg_content = format!(
            "<appmsg appid='wxeb7ec651dd0aefa9' sdkver=''>\
             <title>{}</title>\
             <des></des>\
             <action></action>\
             <type>6</type>\
             <content></content>\
             <url></url>\
             <lowurl></lowurl>\
             <appattach>\
             <totallen>{}</totallen>\
             <attachid>{}</attachid>\
             <fileext>{}</fileext>\
             </appattach>\
             <extinfo></extinfo>\
             </appmsg>",
            file_name, file_size, media_id, file_ext
        );

        let send_body = json!({
            "BaseRequest": base_request,
            "Msg": {
                "Type": 6,
                "Content": app_msg_content,
                "FromUserName": session.username_hash,
                "ToUserName": "filehelper",
                "LocalID": msg_id,
                "ClientMsgId": msg_id
            },
            "Scene": 0
        });

        let send_resp = client
            .post(&format!("{}/cgi-bin/mmwebwx-bin/webwxsendappmsg", WX_FILEHELPER_HOST))
            .query(&params)
            .header(CONTENT_TYPE, "application/json")
            .header("mmweb_appid", APP_ID)
            .json(&send_body)
            .send()
            .await
            .map_err(|e| format!("Send file msg failed: {}", e))?;

        let send_data: Value = send_resp.json().await
            .map_err(|e| format!("Parse send response: {}", e))?;

        let ret = send_data["BaseResponse"]["Ret"].as_i64().unwrap_or(-1);
        if ret != 0 {
            return Err(format!("Send file message failed, Ret={}", ret));
        }
    }

    info!("[{}] File '{}' sent successfully via WeChat", session_id, file_name);

    // Send a text confirmation so user can see the actual send result on phone
    let size_str = if file_size >= 1_048_576 {
        format!("{:.1}MB", file_size as f64 / 1_048_576.0)
    } else {
        format!("{:.1}KB", file_size as f64 / 1024.0)
    };
    let confirm_msg = format!("✅ 文件已发送：{} ({})", file_name, size_str);
    let _ = send_text_message(session_id, &confirm_msg, true).await;

    Ok(format!("文件 '{}' ({}) 已通过微信文件传输助手发送成功，用户手机应可收到", file_name, size_str))
}

async fn sync_check(session_id: &str) -> Result<bool, String> {
    let (session, client) = get_session_snapshot(session_id)?;

    if !session.logged_in {
        return Err("Not logged in".to_string());
    }

    let sync_key_str = if let Some(ref sk) = session.sync_key {
        if let Some(list) = sk["List"].as_array() {
            list.iter()
                .filter_map(|item| {
                    let key = item["Key"].as_i64()?;
                    let val = item["Val"].as_i64()?;
                    Some(format!("{}_{}", key, val))
                })
                .collect::<Vec<_>>()
                .join("|")
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let r_str = now_ms().to_string();
    let device_id = generate_device_id();
    let params = [
        ("r", r_str.as_str()),
        ("skey", session.skey.as_str()),
        ("sid", session.sid.as_str()),
        ("uin", session.uin.as_str()),
        ("deviceid", device_id.as_str()),
        ("synckey", sync_key_str.as_str()),
        ("mmweb_appid", APP_ID),
    ];

    let resp = client
        .get(&format!("{}/cgi-bin/mmwebwx-bin/synccheck", WX_FILEHELPER_HOST))
        .query(&params)
        .header("mmweb_appid", APP_ID)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("synccheck failed: {}", e))?;

    let text = resp.text().await.map_err(|e| format!("Read synccheck: {}", e))?;

    let retcode = regex_match(r#"retcode:"(\d+)""#, &text).unwrap_or_default();
    let selector = regex_match(r#"selector:"(\d+)""#, &text).unwrap_or_default();

    if retcode == "0" && selector != "0" {
        Ok(true)
    } else if retcode == "1100" || retcode == "1101" || retcode == "1102" {
        Err(format!("expired: synccheck retcode={}", retcode))
    } else if retcode != "0" {
        Err(format!("synccheck retcode={}", retcode))
    } else {
        Ok(false)
    }
}

async fn receive_messages(session_id: &str) -> Result<Vec<ChatMessage>, String> {
    let (session, client) = get_session_snapshot(session_id)?;
    let base_request = build_base_request(&session)?;
    let sync_key = session.sync_key.clone().unwrap_or(json!({}));

    let params = [
        ("sid", session.sid.as_str()),
        ("skey", session.skey.as_str()),
        ("pass_ticket", session.pass_ticket.as_str()),
    ];

    let body = json!({
        "BaseRequest": base_request,
        "SyncKey": sync_key
    });

    let resp = client
        .post(&format!("{}/cgi-bin/mmwebwx-bin/webwxsync", WX_FILEHELPER_HOST))
        .query(&params)
        .header(CONTENT_TYPE, "application/json")
        .header("mmweb_appid", APP_ID)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("webwxsync failed: {}", e))?;

    let bytes = resp.bytes().await.map_err(|e| format!("Read sync body: {}", e))?;
    let text = String::from_utf8_lossy(&bytes);
    let data: Value = serde_json::from_str(&text)
        .map_err(|e| format!("Parse sync JSON: {}", e))?;

    let ret = data["BaseResponse"]["Ret"].as_i64().unwrap_or(-1);
    if ret != 0 {
        if ret == 1100 || ret == 1101 || ret == 1102 {
            return Err(format!("expired: webwxsync Ret={}", ret));
        }
        return Err(format!("webwxsync Ret={}", ret));
    }

    // Update sync key
    {
        let mut sessions = SESSIONS.lock().unwrap();
        if let Some(s) = sessions.get_mut(session_id) {
            let has_new_key = !data["SyncKey"].is_null() && data["SyncKey"]["Count"].as_i64().unwrap_or(0) > 0;
            let has_check_key = !data["SyncCheckKey"].is_null() && data["SyncCheckKey"]["Count"].as_i64().unwrap_or(0) > 0;
            
            if has_new_key {
                s.session.sync_key = Some(data["SyncKey"].clone());
            } else if has_check_key {
                s.session.sync_key = Some(data["SyncCheckKey"].clone());
            }
        }
    }
    save_sessions_to_disk();

    // Extract messages
    let mut new_messages = Vec::new();
    if let Some(msg_list) = data["AddMsgList"].as_array() {
        for msg in msg_list {
            let msg_type = msg["MsgType"].as_i64().unwrap_or(0) as i32;
            if msg_type == 1 {
                let raw_content = msg["Content"].as_str().unwrap_or("").to_string();
                let create_time = msg["CreateTime"].as_u64().unwrap_or(0);

                // File helper is a self-chat: ALL messages have the same FromUserName.
                // Use plaintext prefix to distinguish bot replies from user messages.
                let is_bot = raw_content.starts_with(BOT_PREFIX);
                let content = raw_content.clone(); // Keep prefix visible for UI

                info!(
                    "[{}] Sync msg: len={}, is_bot={}, type={}, ts={}",
                    session_id, content.len(), is_bot, msg_type, create_time
                );

                let chat_msg = ChatMessage {
                    content: content.clone(),
                    from_me: true,
                    is_bot,
                    timestamp: create_time,
                    msg_type,
                };

                // Save to database with deduplication (protects against self-echo syncing over previously locally saved messages)
                let _ = database::save_message_dedup(session_id, &content, true, msg_type, is_bot);

                new_messages.push(chat_msg);
            }
        }
    }

    Ok(new_messages)
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Create a new session and return its ID
#[tauri::command]
pub async fn filehelper_create_session() -> Result<Value, String> {
    let session_id = uuid::Uuid::new_v4().to_string();

    {
        let mut sessions = SESSIONS.lock().unwrap();
        sessions.insert(session_id.clone(), SessionState {
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
    
    // Cleanup: remove any session that isn't logged in (garbage collection for stale tabs)
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
    let uuid = generate_qr_uuid(&session_id).await?;
    let qr_url = get_qr_url(&uuid);
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

    let status = check_login_status(&session_id, &uuid).await;
    match status {
        Ok(s) => {
            if s == "logged_in" {
                let nickname = match webwx_init(&session_id).await {
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
    send_text_message(&session_id, &content, false).await?; // User messages from UI are not bots

    // Spawn AI auto-reply in background (since WeChat sync echo gets deduplicated
    // and never triggers auto-reply from the polling loop)
    let sid = session_id.clone();
    let user_content = content.clone();
    tauri::async_runtime::spawn(async move {
        // 1. Send immediate acknowledgment
        let ack_msg = "🫡 收到，正在处理...";
        let _ = send_text_message(&sid, ack_msg, true).await;

        // 2. Process with agent
        match super::agent::agent_process_message(&sid, &user_content).await {
            Ok(reply) => {
                info!("[{}] Agent reply sent from UI message", sid);
                let _ = send_text_message(&sid, &reply, true).await;
            }
            Err(e) => {
                warn!("[{}] Agent auto-reply error: {}", sid, e);
                let err_msg = format!("❌ 执行出错: {}", e);
                let _ = send_text_message(&sid, &err_msg, true).await;
            }
        }
    });

    Ok(json!({ "ok": true }))
}

/// Send a file through the File Transfer Assistant channel
#[tauri::command]
pub async fn filehelper_send_file(session_id: String, file_path: String) -> Result<Value, String> {
    let result = send_file_to_wechat(&session_id, &file_path).await?;
    Ok(json!({ "ok": true, "message": result }))
}

/// Core polling logic — used by both the Tauri command and the background poller.
async fn filehelper_poll_messages_inner(session_id: &str) -> Result<Value, String> {
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
        let has_msg = match sync_check(session_id).await {
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
        let db_count = match database::count_messages(session_id) {
            Ok(c) => c,
            Err(_) => 0,
        };
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
                msgs = match receive_messages(session_id).await {
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
                        let sid = session_id.to_string();
                        let content = msg.content.clone();
                        
                        tauri::async_runtime::spawn(async move {
                            // 1. Send immediate emoji acknowledgment
                            let ack_msg = "🫡 收到，正在处理...";
                            let _ = send_text_message(&sid, ack_msg, true).await;

                            // 2. Use full agent with tools instead of simple AI chat
                            match super::agent::agent_process_message(&sid, &content).await {
                                Ok(reply) => {
                                    info!("[{}] Agent reply sent asynchronously", sid);
                                    let _ = send_text_message(&sid, &reply, true).await;
                                }
                                Err(e) => {
                                    warn!("[{}] Agent auto-reply error: {}", sid, e);
                                    let err_msg = format!("❌ 执行出错: {}", e);
                                    let _ = send_text_message(&sid, &err_msg, true).await;
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
    filehelper_poll_messages_inner(&session_id).await
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
