//! Core WeChat protocol implementation — login, messaging, sync, file upload.
//!
//! This is the SDK layer that talks to WeChat servers directly.

use reqwest::header::CONTENT_TYPE;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

use crate::modules::database;
use super::{
    SESSIONS, save_sessions_to_disk,
    WX_LOGIN_HOST, WX_QR_HOST, WX_FILEHELPER_HOST, APP_ID, BOT_PREFIX,
    now_ms, generate_device_id, generate_msg_id, regex_match,
    get_session_snapshot, build_base_request,
    ChatMessage,
};

// ============================================================================
// Login flow
// ============================================================================

pub(crate) async fn generate_qr_uuid(session_id: &str) -> Result<String, String> {
    // Official filehelper.weixin.qq.com DOUBLE-ENCODES the redirect_uri!
    // First encode: https%3A%2F%2F... → then encode again: https%253A%252F%252F...
    // This is how the server distinguishes file helper login from web WeChat.
    let redirect_uri_raw = format!("{}/cgi-bin/mmwebwx-bin/webwxnewloginpage", WX_FILEHELPER_HOST);
    let redirect_uri_single = urlencoding::encode(&redirect_uri_raw);
    let redirect_uri_double = urlencoding::encode(&redirect_uri_single);
    let now_ts = now_ms();
    
    let url = format!(
        "{}/jslogin?appid={}&redirect_uri={}&fun=new&lang=zh_CN&_={}",
        WX_LOGIN_HOST, APP_ID, redirect_uri_double, now_ts
    );

    let (_, client) = get_session_snapshot(session_id)?;

    info!("[{}] jslogin URL: {}", session_id, &url[..url.len().min(200)]);

    let resp = client
        .get(&url)
        .header("Sec-Fetch-Dest", "script")
        .header("Sec-Fetch-Mode", "no-cors")
        .header("Sec-Fetch-Site", "same-site")
        .send()
        .await
        .map_err(|e| format!("jslogin request failed: {}", e))?;;

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

pub(crate) fn get_qr_url(uuid: &str) -> String {
    format!("{}/qrcode/{}", WX_QR_HOST, uuid)
}

pub(crate) async fn check_login_status(session_id: &str, uuid: &str) -> Result<String, String> {
    let (_, client) = get_session_snapshot(session_id)?;

    // Match Python _poll_login_once: manually construct URL
    let now = now_ms();
    let r_value = !(now as i64 / 1000);
    let uuid_encoded = urlencoding::encode(uuid);

    let url = format!(
        "{}/cgi-bin/mmwebwx-bin/login?loginicon=true&uuid={}&tip=1&r={}&_={}&appid={}",
        WX_LOGIN_HOST, uuid_encoded, r_value, now, APP_ID
    );

    let resp = client
        .get(&url)
        .header("Sec-Fetch-Dest", "script")
        .header("Sec-Fetch-Mode", "no-cors")
        .header("Sec-Fetch-Site", "same-site")
        .timeout(std::time::Duration::from_secs(40))
        .send()
        .await
        .map_err(|e| format!("Login check failed: {}", e))?;;

    let text = resp.text().await.map_err(|e| format!("Read body failed: {}", e))?;
    let code = regex_match(r#"window\.code\s*=\s*(\d+);"#, &text)?;

    match code.as_str() {
        "408" => Err("waiting".to_string()),
        "201" => Ok("scanned".to_string()),
        "200" => {
            let redirect_url = regex_match(r#"window\.redirect_uri="(.*?)";"#, &text)?;
            info!("[{}] Login success! Redirect URL: {}", session_id, &redirect_url[..redirect_url.len().min(100)]);

            // CRITICAL: Official filehelper.weixin.qq.com constructs this URL by
            // taking the redirect URL as-is and prepending fun=new&version=v2.
            // It does NOT re-encode the query params. Using .query() or even
            // Url::parse would encode @ to %40 and == to %3D%3D, causing 1203.
            // So we use RAW string manipulation to preserve exact encoding.
            let (base_url, raw_query) = if let Some(pos) = redirect_url.find('?') {
                (&redirect_url[..pos], &redirect_url[pos+1..])
            } else {
                (redirect_url.as_str(), "")
            };
            
            // Build URL exactly like official site: fun=new&version=v2 FIRST, then original params
            let login_page_url = format!(
                "{}?fun=new&version=v2&{}",
                base_url, raw_query
            );

            info!("[{}] webwxnewloginpage full URL: {}", session_id, &login_page_url[..login_page_url.len().min(300)]);

            let resp = client
                .get(&login_page_url)
                .header("mmweb_appid", APP_ID)
                .header("Sec-Fetch-Dest", "empty")
                .header("Sec-Fetch-Mode", "cors")
                .header("Sec-Fetch-Site", "same-origin")
                .send()
                .await
                .map_err(|e| format!("webwxnewloginpage failed: {}", e))?;

            info!("[{}] webwxnewloginpage response: status={}, final_url={}",
                session_id, resp.status(), resp.url());

            let mut captured_cookies = Vec::new();
            for header in resp.headers().get_all(reqwest::header::SET_COOKIE) {
                if let Ok(c_str) = header.to_str() {
                    captured_cookies.push(c_str.to_string());
                }
            }

            let body = resp.text().await.map_err(|e| format!("Read redirect body: {}", e))?;
            info!("[{}] Login redirect body (first 500 chars): {}", session_id, &body[..body.len().min(500)]);

            let skey = regex_match(r"<skey>(.*?)</skey>", &body).unwrap_or_default();
            let wxsid = regex_match(r"<wxsid>(.*?)</wxsid>", &body).unwrap_or_default();
            let wxuin = regex_match(r"<wxuin>(.*?)</wxuin>", &body).unwrap_or_default();
            let pass_ticket = regex_match(r"<pass_ticket>(.*?)</pass_ticket>", &body).unwrap_or_default();
            
            // If all of these are empty, we probably hit a redirect error page
            if skey.is_empty() && wxsid.is_empty() && wxuin.is_empty() {
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
                    // Save the server-assigned API host from the redirect URL
                    // (e.g. "szfilehelper.weixin.qq.com")
                    let (base_url_part, _) = if let Some(pos) = redirect_url.find('?') {
                        (&redirect_url[..pos], &redirect_url[pos+1..])
                    } else {
                        (redirect_url.as_str(), "")
                    };
                    if let Ok(parsed_redir) = reqwest::Url::parse(base_url_part) {
                        if let Some(host) = parsed_redir.host_str() {
                            s.session.api_host = format!("https://{}", host);
                            info!("[{}] API host set to: {}", session_id, s.session.api_host);
                        }
                    }
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

pub(crate) async fn webwx_init(session_id: &str) -> Result<String, String> {
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
        .post(&format!("{}/cgi-bin/mmwebwx-bin/webwxinit", session.api_host_url()))
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
        .post(&format!("{}/cgi-bin/mmwebwx-bin/webwxsendmsg", session.api_host_url()))
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

    // Save to database immediately so it shows up in history
    let _ = database::save_message(session_id, content, true, 1, is_bot);

    Ok(())
}

/// Upload a file to WeChat servers and send it through the File Transfer Assistant channel.
pub async fn send_file_to_wechat(session_id: &str, file_path: &str) -> Result<String, String> {
    const WX_FILEUPLOAD_HOST: &str = "https://file.wx.qq.com";

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

    // Compute MD5
    let md5_output = std::process::Command::new("md5")
        .arg("-q").arg(file_path)
        .output()
        .map_err(|e| format!("md5 command failed: {}", e))?;
    let file_md5 = String::from_utf8_lossy(&md5_output.stdout).trim().to_string();

    let is_image = matches!(file_ext.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp");
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

    // Step 1: Upload via multipart
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
        .header("Origin", session.api_host_url())
        .header("Referer", session.api_host_url())
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
            .post(&format!("{}/cgi-bin/mmwebwx-bin/webwxsendmsgimg", session.api_host_url()))
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
            .post(&format!("{}/cgi-bin/mmwebwx-bin/webwxsendappmsg", session.api_host_url()))
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

    let size_str = if file_size >= 1_048_576 {
        format!("{:.1}MB", file_size as f64 / 1_048_576.0)
    } else {
        format!("{:.1}KB", file_size as f64 / 1024.0)
    };
    let confirm_msg = format!("✅ 文件已发送：{} ({})", file_name, size_str);
    let _ = send_text_message(session_id, &confirm_msg, true).await;

    Ok(format!("文件 '{}' ({}) 已通过微信文件传输助手发送成功，用户手机应可收到", file_name, size_str))
}

// ============================================================================
// Sync & receive
// ============================================================================

pub async fn sync_check(session_id: &str) -> Result<bool, String> {
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
        .get(&format!("{}/cgi-bin/mmwebwx-bin/synccheck", session.api_host_url()))
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

pub(crate) async fn receive_messages(session_id: &str) -> Result<Vec<ChatMessage>, String> {
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
        .post(&format!("{}/cgi-bin/mmwebwx-bin/webwxsync", session.api_host_url()))
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
            let raw_content = msg["Content"].as_str().unwrap_or("").to_string();
            let create_time = msg["CreateTime"].as_u64().unwrap_or(0);
            let msg_id_str = msg["MsgId"].as_str().unwrap_or("").to_string();

            match msg_type {
                // Text message
                1 => {
                    let is_bot = raw_content.starts_with(BOT_PREFIX);
                    let content = raw_content.clone();

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

                    let _ = database::save_message_dedup(session_id, &content, true, msg_type, is_bot);
                    new_messages.push(chat_msg);
                }

                // Image message
                3 => {
                    info!("[{}] Sync image msg: MsgId={}", session_id, msg_id_str);

                    // Auto-download image
                    let sid = session_id.to_string();
                    let mid = msg_id_str.clone();
                    tokio::spawn(async move {
                        match download_message_content(&sid, &mid, 3, None, None, None).await {
                            Ok(path) => info!("[{}] Auto-downloaded image: {}", sid, path),
                            Err(e) => warn!("[{}] Image download failed: {}", sid, e),
                        }
                    });

                    let chat_msg = ChatMessage {
                        content: format!("[图片] {}", msg_id_str),
                        from_me: true,
                        is_bot: false,
                        timestamp: create_time,
                        msg_type,
                    };

                    let _ = database::save_message_dedup(session_id, &chat_msg.content, true, msg_type, false);
                    new_messages.push(chat_msg);
                }

                // App message (file attachment — AppMsgType=6)
                49 => {
                    let app_msg_type = msg["AppMsgType"].as_i64().unwrap_or(0);
                    if app_msg_type == 6 {
                        let file_name = msg["FileName"].as_str().unwrap_or("file").to_string();
                        info!("[{}] Sync file msg: MsgId={}, name={}", session_id, msg_id_str, file_name);

                        let from_user = msg["FromUserName"].as_str().unwrap_or("").to_string();
                        let media_id = msg["MediaId"].as_str().unwrap_or("").to_string();
                        let encry_file_name = msg["EncryFileName"].as_str().unwrap_or("").to_string();

                        // Auto-download file
                        let sid = session_id.to_string();
                        let mid = msg_id_str.clone();
                        let fname = file_name.clone();
                        tokio::spawn(async move {
                            match download_message_content(
                                &sid, &mid, 49,
                                Some(&from_user), Some(&media_id), Some(&encry_file_name),
                            ).await {
                                Ok(path) => info!("[{}] Auto-downloaded file '{}': {}", sid, fname, path),
                                Err(e) => warn!("[{}] File download failed for '{}': {}", sid, fname, e),
                            }
                        });

                        let content = format!("[文件] {}", file_name);
                        let chat_msg = ChatMessage {
                            content: content.clone(),
                            from_me: true,
                            is_bot: false,
                            timestamp: create_time,
                            msg_type,
                        };

                        let _ = database::save_message_dedup(session_id, &content, true, msg_type, false);
                        new_messages.push(chat_msg);
                    }
                }

                _ => {
                    // Other message types — log but skip
                }
            }
        }
    }

    Ok(new_messages)
}

/// Download a received file from WeChat servers.
///
/// - msg_type=3 → image → webwxgetmsgimg
/// - msg_type=49 (with from_user, media_id, encry_file_name) → file → webwxgetmedia
pub async fn download_message_content(
    session_id: &str,
    msg_id: &str,
    msg_type: i32,
    from_user: Option<&str>,
    media_id: Option<&str>,
    encry_file_name: Option<&str>,
) -> Result<String, String> {
    let (session, client) = get_session_snapshot(session_id)?;
    if !session.logged_in {
        return Err("Not logged in".to_string());
    }

    let url = match msg_type {
        3 => {
            // Image download
            format!(
                "{}/cgi-bin/mmwebwx-bin/webwxgetmsgimg?MsgID={}&skey={}&type=slave&mmweb_appid={}",
                WX_FILEHELPER_HOST,
                msg_id,
                urlencoding::encode(&session.skey),
                APP_ID,
            )
        }
        49 => {
            // File download
            let from = from_user.ok_or("from_user required for file download")?;
            let mid = media_id.ok_or("media_id required for file download")?;
            let efn = encry_file_name.unwrap_or("");

            format!(
                "https://file.wx.qq.com/cgi-bin/mmwebwx-bin/webwxgetmedia\
                 ?sender={}&mediaid={}&encryfilename={}&fromuser={}\
                 &pass_ticket={}&webwx_data_ticket={}&sid={}&mmweb_appid={}",
                urlencoding::encode(from),
                urlencoding::encode(mid),
                urlencoding::encode(efn),
                urlencoding::encode(&session.uin),
                urlencoding::encode(&session.pass_ticket),
                urlencoding::encode(&session.webwx_data_ticket),
                urlencoding::encode(&session.sid),
                APP_ID,
            )
        }
        _ => return Err(format!("Unsupported msg_type for download: {}", msg_type)),
    };

    info!("[{}] Downloading {} content, msg_id={}", session_id, if msg_type == 3 { "image" } else { "file" }, msg_id);

    let resp = client
        .get(&url)
        .header("mmweb_appid", APP_ID)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| format!("Download request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Download failed with status: {}", resp.status()));
    }

    let bytes = resp.bytes().await
        .map_err(|e| format!("Read download body: {}", e))?;

    // Save to downloads directory
    let data_dir = crate::modules::config::get_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let download_dir = data_dir.join("downloads");
    std::fs::create_dir_all(&download_dir).ok();

    // Date subdirectory
    let date_str = chrono::Local::now().format("%Y%m%d").to_string();
    let date_dir = download_dir.join(&date_str);
    std::fs::create_dir_all(&date_dir).ok();

    let ext = if msg_type == 3 { ".jpg" } else { "" };
    let file_name = format!("{}_{}{}", 
        if msg_type == 3 { "img" } else { "file" },
        &msg_id[..msg_id.len().min(16)],
        ext,
    );
    let save_path = date_dir.join(&file_name);

    std::fs::write(&save_path, &bytes)
        .map_err(|e| format!("Failed to save downloaded file: {}", e))?;

    let save_path_str = save_path.to_string_lossy().to_string();
    let file_size = bytes.len() as i64;

    // Save file metadata to DB
    let _ = database::save_file(session_id, Some(msg_id), &file_name, &save_path_str, file_size, None);

    info!("[{}] Downloaded {} ({} bytes) → {}", session_id, file_name, file_size, save_path_str);

    Ok(save_path_str)
}

