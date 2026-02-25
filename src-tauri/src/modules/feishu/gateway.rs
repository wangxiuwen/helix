//! Feishu WebSocket Gateway — event subscription for receiving messages.
//!
//! Connects to the Feishu long-connection WebSocket endpoint and listens
//! for `im.message.receive_v1` events. Routes incoming messages to the
//! AI Agent for auto-reply.
//!
//! Feishu WS v2 uses a protobuf frame format (pbbp2.Frame), not JSON text.

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::watch;
use tracing::{info, warn, error};

use super::{FEISHU_STATE, api};

// ============================================================================
// Protobuf frame decoder for pbbp2.Frame
// ============================================================================

/// Decoded protobuf Frame from Feishu WebSocket v2.
#[derive(Debug, Default)]
struct PbFrame {
    seq_id: u64,
    _log_id: u64,
    service: i32,
    method: i32,          // 0 = control, 1 = data
    headers: Vec<(String, String)>,
    payload_encoding: String,
    _payload_type: String,
    payload: Vec<u8>,
    _log_id_new: String,
}

/// Minimal protobuf varint decoder.
fn read_varint(data: &[u8], pos: &mut usize) -> u64 {
    let mut result: u64 = 0;
    let mut shift = 0;
    while *pos < data.len() {
        let byte = data[*pos];
        *pos += 1;
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    result
}

/// Read a length-delimited field (string or bytes).
fn read_bytes(data: &[u8], pos: &mut usize) -> Vec<u8> {
    let len = read_varint(data, pos) as usize;
    let end = (*pos + len).min(data.len());
    let result = data[*pos..end].to_vec();
    *pos = end;
    result
}

/// Decode a pbbp2.Header { key=1, value=2 }.
fn decode_header(data: &[u8]) -> (String, String) {
    let mut pos = 0;
    let mut key = String::new();
    let mut value = String::new();
    while pos < data.len() {
        let tag = read_varint(data, &mut pos);
        let field = tag >> 3;
        let wire_type = tag & 0x7;
        match (field, wire_type) {
            (1, 2) => key = String::from_utf8_lossy(&read_bytes(data, &mut pos)).to_string(),
            (2, 2) => value = String::from_utf8_lossy(&read_bytes(data, &mut pos)).to_string(),
            (_, 0) => { read_varint(data, &mut pos); }
            (_, 2) => { read_bytes(data, &mut pos); }
            _ => break,
        }
    }
    (key, value)
}

/// Decode a pbbp2.Frame from raw binary data.
fn decode_frame(data: &[u8]) -> PbFrame {
    let mut frame = PbFrame::default();
    let mut pos = 0;
    while pos < data.len() {
        let tag = read_varint(data, &mut pos);
        let field = tag >> 3;
        let wire_type = tag & 0x7;
        match (field, wire_type) {
            (1, 0) => frame.seq_id = read_varint(data, &mut pos),
            (2, 0) => frame._log_id = read_varint(data, &mut pos),
            (3, 0) => frame.service = read_varint(data, &mut pos) as i32,
            (4, 0) => frame.method = read_varint(data, &mut pos) as i32,
            (5, 2) => {
                let header_bytes = read_bytes(data, &mut pos);
                frame.headers.push(decode_header(&header_bytes));
            }
            (6, 2) => frame.payload_encoding = String::from_utf8_lossy(&read_bytes(data, &mut pos)).to_string(),
            (7, 2) => frame._payload_type = String::from_utf8_lossy(&read_bytes(data, &mut pos)).to_string(),
            (8, 2) => frame.payload = read_bytes(data, &mut pos),
            (9, 2) => frame._log_id_new = String::from_utf8_lossy(&read_bytes(data, &mut pos)).to_string(),
            (_, 0) => { read_varint(data, &mut pos); }
            (_, 2) => { read_bytes(data, &mut pos); }
            _ => break,
        }
    }
    frame
}

/// Get a header value from a frame.
fn get_header<'a>(frame: &'a PbFrame, key: &str) -> Option<&'a str> {
    frame.headers.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

// ---------- Protobuf encoder ----------

fn write_varint(val: u64, out: &mut Vec<u8>) {
    let mut v = val;
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 { byte |= 0x80; }
        out.push(byte);
        if v == 0 { break; }
    }
}

fn encode_header_pb(key: &str, value: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    // field 1 (key), wire type 2
    write_varint(10, &mut buf);
    write_varint(key.len() as u64, &mut buf);
    buf.extend_from_slice(key.as_bytes());
    // field 2 (value), wire type 2
    write_varint(18, &mut buf);
    write_varint(value.len() as u64, &mut buf);
    buf.extend_from_slice(value.as_bytes());
    buf
}

fn encode_frame(frame: &PbFrame) -> Vec<u8> {
    let mut buf = Vec::new();
    // field 1: SeqID (varint)
    write_varint(8, &mut buf); write_varint(frame.seq_id, &mut buf);
    // field 2: LogID (varint)
    write_varint(16, &mut buf); write_varint(frame._log_id, &mut buf);
    // field 3: service (varint)
    write_varint(24, &mut buf); write_varint(frame.service as u64, &mut buf);
    // field 4: method (varint)
    write_varint(32, &mut buf); write_varint(frame.method as u64, &mut buf);
    // field 5: headers (repeated, length-delimited)
    for (k, v) in &frame.headers {
        let hdr = encode_header_pb(k, v);
        write_varint(42, &mut buf);
        write_varint(hdr.len() as u64, &mut buf);
        buf.extend_from_slice(&hdr);
    }
    buf
}

/// Build a ping frame for the given service_id.
fn build_ping_frame(service_id: &str) -> Vec<u8> {
    let frame = PbFrame {
        service: service_id.parse::<i32>().unwrap_or(0),
        method: 0, // control
        headers: vec![("type".to_string(), "ping".to_string())],
        ..Default::default()
    };
    encode_frame(&frame)
}

/// Build a protobuf ack frame to acknowledge a data frame.
/// Without this ack, the server will keep retransmitting the same event.
fn build_ack_frame(service_id: &str) -> Vec<u8> {
    let ack_payload = b"{\"code\":200}";
    let frame = PbFrame {
        service: service_id.parse::<i32>().unwrap_or(0),
        method: 1, // data
        headers: vec![("type".to_string(), "event".to_string())],
        payload: ack_payload.to_vec(),
        ..Default::default()
    };
    encode_frame(&frame)
}

// ============================================================================
// Event types
// ============================================================================

#[derive(Debug, Deserialize)]
struct WsFrame {
    #[serde(default)]
    r#type: String, // "event", "ping", etc.
    #[serde(default)]
    header: Option<WsEventHeader>,
    #[serde(default)]
    event: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct WsEventHeader {
    #[serde(default)]
    event_type: String,
    #[serde(default)]
    event_id: String,
}

#[derive(Debug, Clone)]
pub struct FeishuIncomingMessage {
    pub message_id: String,
    pub chat_id: String,
    pub chat_type: String, // "p2p" or "group"
    pub sender_id: String,
    pub message_type: String,
    pub content: String,
    pub create_time: String,
}

// ============================================================================
// Deduplication
// ============================================================================

use std::collections::HashSet;
use std::sync::Mutex;
use once_cell::sync::Lazy;

static SEEN_MESSAGES: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));
const MAX_SEEN: usize = 1000;

fn is_duplicate(msg_id: &str) -> bool {
    let mut seen = SEEN_MESSAGES.lock().unwrap();
    if seen.contains(msg_id) {
        return true;
    }
    if seen.len() >= MAX_SEEN {
        seen.clear();
    }
    seen.insert(msg_id.to_string());
    false
}

// ============================================================================
// Gateway control
// ============================================================================

/// Start the Feishu WebSocket gateway.
pub async fn start_gateway() -> Result<(), String> {
    // Check if already connected
    {
        let state = FEISHU_STATE.lock().await;
        if state.gateway_connected {
            return Err("Feishu gateway already running".to_string());
        }
        if state.config.app_id.is_empty() || state.config.app_secret.is_empty() {
            return Err("Feishu appId/appSecret not configured".to_string());
        }
    }

    let (abort_tx, abort_rx) = watch::channel(false);

    // Store abort sender
    {
        let mut state = FEISHU_STATE.lock().await;
        state.gateway_abort = Some(abort_tx);
        state.gateway_connected = true;
    }

    // Spawn the gateway loop
    tauri::async_runtime::spawn(async move {
        gateway_loop(abort_rx).await;
    });

    info!("[Feishu] Gateway started");
    Ok(())
}

/// Stop the Feishu WebSocket gateway.
pub async fn stop_gateway() -> Result<(), String> {
    let mut state = FEISHU_STATE.lock().await;
    if let Some(abort_tx) = state.gateway_abort.take() {
        let _ = abort_tx.send(true);
    }
    state.gateway_connected = false;
    info!("[Feishu] Gateway stopped");
    Ok(())
}

// ============================================================================
// Gateway loop with reconnection
// ============================================================================

async fn gateway_loop(mut abort_rx: watch::Receiver<bool>) {
    let mut reconnect_delay = 3u64;
    let max_delay = 60u64;

    loop {
        // Check abort
        if *abort_rx.borrow() {
            info!("[Feishu] Gateway abort signal received");
            break;
        }

        match run_ws_connection(&mut abort_rx).await {
            Ok(()) => {
                info!("[Feishu] WebSocket connection closed normally");
                reconnect_delay = 3;
            }
            Err(e) => {
                error!("[Feishu] WebSocket connection error: {}", e);
                // Don't retry on config-level errors
                if e.contains("9499") || e.contains("not configured") || e.contains("appId") {
                    error!("[Feishu] Fatal config error, stopping gateway");
                    break;
                }
            }
        }

        // Check abort before reconnect
        if *abort_rx.borrow() {
            break;
        }

        warn!("[Feishu] Reconnecting in {}s...", reconnect_delay);
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(reconnect_delay)) => {},
            _ = abort_rx.changed() => {
                if *abort_rx.borrow() { break; }
            }
        }

        reconnect_delay = (reconnect_delay * 2).min(max_delay);
    }

    // Mark disconnected
    let mut state = FEISHU_STATE.lock().await;
    state.gateway_connected = false;
    info!("[Feishu] Gateway loop ended");
}

// ============================================================================
// WebSocket connection
// ============================================================================

async fn run_ws_connection(abort_rx: &mut watch::Receiver<bool>) -> Result<(), String> {
    // Get WS endpoint
    info!("[Feishu] Requesting WebSocket endpoint...");
    let ws_data = api::get_ws_endpoint().await?;
    let ws_url = &ws_data.url;
    info!("[Feishu] Full WS URL: {}", ws_url);

    // Extract service_id and device_id from the URL query params
    let service_id = ws_url.split("service_id=").nth(1)
        .and_then(|s| s.split('&').next())
        .unwrap_or("0")
        .to_string();
    info!("[Feishu] service_id={}", service_id);

    // Get ping interval from ClientConfig (default 120s)
    let ping_interval_secs = ws_data.client_config
        .as_ref()
        .and_then(|c| c.get("PingInterval"))
        .and_then(|v| v.as_u64())
        .unwrap_or(120);
    info!("[Feishu] ping_interval={}s", ping_interval_secs);

    // Connect using tokio-tungstenite
    let (ws_stream, _) = tokio_tungstenite::connect_async(ws_url)
        .await
        .map_err(|e| format!("WebSocket connect failed: {}", e))?;

    info!("[Feishu] WebSocket connected!");

    use futures::StreamExt;
    use futures::SinkExt;
    use tokio_tungstenite::tungstenite::Message;

    let (mut ws_sink, mut ws_stream) = ws_stream.split();

    // Send initial protobuf ping (required for Feishu to route events)
    let ping_data = build_ping_frame(&service_id);
    info!("[Feishu] Sending protobuf ping ({}B): {}", ping_data.len(),
        ping_data.iter().map(|b| format!("{:02x}", b)).collect::<String>());
    match ws_sink.send(Message::Binary(ping_data)).await {
        Ok(_) => info!("[Feishu] Protobuf ping sent successfully"),
        Err(e) => {
            error!("[Feishu] Failed to send protobuf ping: {}", e);
            return Err(format!("Ping send failed: {}", e));
        }
    }

    let mut ping_timer = tokio::time::interval(std::time::Duration::from_secs(ping_interval_secs));
    ping_timer.tick().await; // consume first immediate tick

    loop {
        tokio::select! {
            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        info!("[Feishu] WS Text frame ({}B): {}", text.len(), &text[..text.len().min(300)]);
                        handle_ws_message(&text, &mut ws_sink).await;
                    }
                    Some(Ok(Message::Binary(data))) => {
                        let frame = decode_frame(&data);
                        let msg_type = get_header(&frame, "type").unwrap_or("").to_string();

                        match frame.method {
                            0 => {
                                // Control frame (ping/pong)
                                if msg_type == "pong" {
                                    // Server acknowledged our ping — connection alive
                                }
                            }
                            1 => {
                                // Data frame — payload contains event JSON
                                info!("[Feishu] Data frame: method=1, headers={:?}, payload_len={}", frame.headers, frame.payload.len());

                                // Send ack immediately to stop server retransmission
                                let ack_data = build_ack_frame(&service_id);
                                if let Err(e) = ws_sink.send(Message::Binary(ack_data)).await {
                                    warn!("[Feishu] Failed to send ack: {}", e);
                                }

                                let payload_str = if frame.payload_encoding == "gzip" {
                                    use std::io::Read;
                                    let mut decoder = flate2::read::GzDecoder::new(&frame.payload[..]);
                                    let mut s = String::new();
                                    decoder.read_to_string(&mut s).unwrap_or_default();
                                    s
                                } else {
                                    String::from_utf8_lossy(&frame.payload).to_string()
                                };

                                if !payload_str.is_empty() {
                                    info!("[Feishu] Event payload: {}", &payload_str[..payload_str.len().min(500)]);
                                    handle_ws_message(&payload_str, &mut ws_sink).await;
                                }
                            }
                            _ => {
                                info!("[Feishu] Unknown method={} type={}", frame.method, msg_type);
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws_sink.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("[Feishu] WebSocket server closed connection");
                        return Ok(());
                    }
                    Some(Err(e)) => {
                        return Err(format!("WebSocket error: {}", e));
                    }
                    None => {
                        return Ok(());
                    }
                    Some(Ok(Message::Pong(data))) => {
                        info!("[Feishu] WS Pong frame ({}B)", data.len());
                    }
                    Some(Ok(other)) => {
                        info!("[Feishu] WS other frame: {:?}", other);
                    }
                }
            }
            _ = ping_timer.tick() => {
                let ping_data = build_ping_frame(&service_id);
                if let Err(e) = ws_sink.send(Message::Binary(ping_data)).await {
                    error!("[Feishu] Failed to send ping: {}", e);
                    return Err(format!("Ping send failed: {}", e));
                }
                info!("[Feishu] Sent periodic protobuf ping");
            }
            _ = abort_rx.changed() => {
                if *abort_rx.borrow() {
                    info!("[Feishu] Abort received, closing WebSocket");
                    let _ = ws_sink.send(Message::Close(None)).await;
                    return Ok(());
                }
            }
        }
    }
}

// ============================================================================
// Message handler
// ============================================================================

async fn handle_ws_message<S>(text: &str, _sink: &mut S)
where
    S: futures::Sink<tokio_tungstenite::tungstenite::Message> + Unpin,
{
    // The protobuf-decoded payload is a direct event JSON:
    // {"schema":"2.0","header":{"event_type":"im.message.receive_v1",...},"event":{...}}
    let payload: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            warn!("[Feishu] Failed to parse event JSON: {} | text={}", e, &text[..text.len().min(200)]);
            return;
        }
    };

    // Extract event_type from header
    let event_type = payload
        .get("header")
        .and_then(|h| h.get("event_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match event_type {
        "im.message.receive_v1" => {
            if let Some(event) = payload.get("event") {
                handle_message_event(event).await;
            }
        }
        "" => {
            // Try legacy WsFrame format (type field at top level)
            if let Some(frame_type) = payload.get("type").and_then(|v| v.as_str()) {
                match frame_type {
                    "event" => {
                        let inner_event_type = payload
                            .get("header")
                            .and_then(|h| h.get("event_type"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if inner_event_type == "im.message.receive_v1" {
                            if let Some(event) = payload.get("event") {
                                handle_message_event(event).await;
                            }
                        }
                    }
                    "ping" => {} // heartbeat
                    other => info!("[Feishu] Unknown frame type: {}", other),
                }
            } else {
                info!("[Feishu] Unknown payload format: {}", &text[..text.len().min(200)]);
            }
        }
        other => {
            info!("[Feishu] Ignoring event type: {}", other);
        }
    }
}

async fn handle_message_event(event: &Value) {
    // Parse message from event
    let message = match event.get("message") {
        Some(m) => m,
        None => {
            warn!("[Feishu] Event missing 'message' field");
            return;
        }
    };

    let message_id = message.get("message_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let chat_id = message.get("chat_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let chat_type = message.get("chat_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let message_type = message.get("message_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let content_raw = message.get("content").and_then(|v| v.as_str()).unwrap_or("{}");
    let _create_time = message.get("create_time").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // Sender
    let sender = event.get("sender").unwrap_or(&Value::Null);
    let sender_id = sender
        .get("sender_id")
        .and_then(|s| s.get("open_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Dedup
    if message_id.is_empty() || is_duplicate(&message_id) {
        return;
    }

    // Parse content (JSON string → extract text)
    let text = if message_type == "text" {
        let content: Value = serde_json::from_str(content_raw).unwrap_or(json!({}));
        content.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string()
    } else {
        format!("[不支持的消息类型: {}]", message_type)
    };

    if text.is_empty() {
        return;
    }

    info!(
        "[Feishu] Incoming: chat={} type={} sender={} text=\"{}\"",
        chat_id, chat_type, sender_id, &text[..text.len().min(80)]
    );

    // Save to database
    let account_id = format!("feishu:{}", chat_id);
    let _ = crate::modules::database::save_message(&account_id, &text, false, 1, false);

    // Spawn AI agent reply
    let msg_id = message_id.clone();
    let cid = chat_id.clone();
    tauri::async_runtime::spawn(async move {
        // Add typing reaction to original message (like OpenClaw's keyboard emoji)
        let reaction_id = api::add_reaction(&msg_id, "OnIt").await.unwrap_or_default();

        // Process with AI agent
        match crate::modules::agent::agent_process_message(&account_id, &text).await {
            Ok(reply) => {
                info!("[Feishu] Agent reply for chat={}", cid);
                // Save reply to DB
                let _ = crate::modules::database::save_message(&account_id, &reply, true, 1, true);
                // Send reply as direct message
                if let Err(e) = api::send_text_message(&cid, &reply).await {
                    error!("[Feishu] Failed to send reply: {}", e);
                }
            }
            Err(e) => {
                warn!("[Feishu] Agent error for chat={}: {}", cid, e);
                let err_msg = format!("❌ 处理出错: {}", e);
                let _ = api::send_text_message(&cid, &err_msg).await;
            }
        }

        // Remove typing reaction after reply is sent
        if !reaction_id.is_empty() {
            let _ = api::delete_reaction(&msg_id, &reaction_id).await;
        }
    });
}
