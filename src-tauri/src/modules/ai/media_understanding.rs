//! Media Understanding — Image/audio/video understanding and file content extraction.
//!
//! Ported from OpenClaw `src/media-understanding/`: detects media in messages,
//! sends images to vision models, transcribes audio, and inlines text file content.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use base64::Engine as _;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaResult {
    pub media_type: String, // "image", "audio", "video", "text_file"
    pub source: String,     // file path or URL
    pub description: String,
    pub content_length: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaUnderstandingResult {
    pub results: Vec<MediaResult>,
    pub prompt_injection: String,
}

// ============================================================================
// MIME Type Detection
// ============================================================================

/// Known text-like MIME types that should be inlined.
const TEXT_MIMES: &[&str] = &[
    "text/plain", "text/csv", "text/html", "text/xml", "text/yaml",
    "application/json", "application/xml", "application/yaml",
    "application/javascript", "application/typescript",
    "application/x-sh", "application/x-python",
];

/// Detect MIME type from file extension.
pub fn detect_mime(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg".into(),
        "png" => "image/png".into(),
        "gif" => "image/gif".into(),
        "webp" => "image/webp".into(),
        "svg" => "image/svg+xml".into(),
        "bmp" => "image/bmp".into(),
        "ico" => "image/x-icon".into(),
        "mp3" => "audio/mpeg".into(),
        "wav" => "audio/wav".into(),
        "ogg" => "audio/ogg".into(),
        "flac" => "audio/flac".into(),
        "m4a" => "audio/mp4".into(),
        "mp4" => "video/mp4".into(),
        "webm" => "video/webm".into(),
        "avi" => "video/avi".into(),
        "mov" => "video/quicktime".into(),
        "mkv" => "video/x-matroska".into(),
        "json" => "application/json".into(),
        "xml" => "application/xml".into(),
        "yaml" | "yml" => "application/yaml".into(),
        "csv" => "text/csv".into(),
        "txt" | "log" => "text/plain".into(),
        "md" => "text/markdown".into(),
        "py" => "application/x-python".into(),
        "rs" => "text/x-rust".into(),
        "ts" | "tsx" => "application/typescript".into(),
        "js" | "jsx" => "application/javascript".into(),
        "sh" | "bash" | "zsh" => "application/x-sh".into(),
        "toml" => "application/toml".into(),
        "html" | "htm" => "text/html".into(),
        "css" => "text/css".into(),
        "sql" => "application/sql".into(),
        "pdf" => "application/pdf".into(),
        _ => "application/octet-stream".into(),
    }
}

/// Check if a MIME type is a text-like file that can be inlined.
pub fn is_text_mime(mime: &str) -> bool {
    mime.starts_with("text/") || TEXT_MIMES.contains(&mime)
        || mime.contains("json") || mime.contains("xml")
        || mime.contains("yaml") || mime.contains("javascript")
        || mime.contains("typescript") || mime.contains("python")
        || mime.contains("sh") || mime.contains("sql")
        || mime.contains("toml") || mime.contains("rust")
}

/// Check if a MIME type is an image.
pub fn is_image_mime(mime: &str) -> bool {
    mime.starts_with("image/")
}

/// Check if a MIME type is audio.
pub fn is_audio_mime(mime: &str) -> bool {
    mime.starts_with("audio/")
}

/// Check if a MIME type is video.
pub fn is_video_mime(mime: &str) -> bool {
    mime.starts_with("video/")
}

// ============================================================================
// File Content Extraction
// ============================================================================

/// Max file size for inline content (100KB).
const MAX_INLINE_SIZE: u64 = 100 * 1024;

/// Extract text content from a file for inline injection.
pub fn extract_file_content(path: &str, max_chars: usize) -> MediaResult {
    let mime = detect_mime(path);
    let file_path = Path::new(path);

    if !file_path.exists() {
        return MediaResult {
            media_type: "text_file".into(),
            source: path.into(),
            description: String::new(),
            content_length: 0,
            error: Some("File not found".into()),
        };
    }

    let meta = match std::fs::metadata(file_path) {
        Ok(m) => m,
        Err(e) => {
            return MediaResult {
                media_type: "text_file".into(),
                source: path.into(),
                description: String::new(),
                content_length: 0,
                error: Some(format!("Cannot read metadata: {}", e)),
            };
        }
    };

    if meta.len() > MAX_INLINE_SIZE {
        return MediaResult {
            media_type: "text_file".into(),
            source: path.into(),
            description: format!("[文件过大: {} bytes, 上限: {} bytes]", meta.len(), MAX_INLINE_SIZE),
            content_length: meta.len() as usize,
            error: Some("File too large for inline".into()),
        };
    }

    if !is_text_mime(&mime) {
        return MediaResult {
            media_type: "binary_file".into(),
            source: path.into(),
            description: format!("[二进制文件: {}, {} bytes]", mime, meta.len()),
            content_length: meta.len() as usize,
            error: None,
        };
    }

    match std::fs::read_to_string(file_path) {
        Ok(content) => {
            let total_len = content.len();
            let truncated = if content.len() > max_chars {
                format!("{}...\n[截断，共 {} 字符]", &content[..max_chars], total_len)
            } else {
                content
            };

            let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
            let description = format!("<file name=\"{}\" type=\"{}\">\n{}\n</file>", filename, mime, truncated);

            MediaResult {
                media_type: "text_file".into(),
                source: path.into(),
                description,
                content_length: total_len,
                error: None,
            }
        }
        Err(e) => MediaResult {
            media_type: "text_file".into(),
            source: path.into(),
            description: String::new(),
            content_length: 0,
            error: Some(format!("Read error: {}", e)),
        },
    }
}

// ============================================================================
// Image Understanding (Vision API)
// ============================================================================

/// Describe an image using the configured vision model.
pub async fn describe_image(image_path: &str) -> MediaResult {
    let path = Path::new(image_path);
    if !path.exists() {
        return MediaResult {
            media_type: "image".into(),
            source: image_path.into(),
            description: String::new(),
            content_length: 0,
            error: Some("Image file not found".into()),
        };
    }

    // Read image and base64 encode
    let image_data = match std::fs::read(path) {
        Ok(data) => data,
        Err(e) => {
            return MediaResult {
                media_type: "image".into(),
                source: image_path.into(),
                description: String::new(),
                content_length: 0,
                error: Some(format!("Read image: {}", e)),
            };
        }
    };

    let mime = detect_mime(image_path);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&image_data);

    // Use the configured AI model for vision
    let config = match crate::modules::config::load_app_config() {
        Ok(c) => c,
        Err(e) => {
            return MediaResult {
                media_type: "image".into(),
                source: image_path.into(),
                description: String::new(),
                content_length: image_data.len(),
                error: Some(format!("Config: {}", e)),
            };
        }
    };

    let ai = &config.ai_config;
    if ai.api_key.is_empty() {
        return MediaResult {
            media_type: "image".into(),
            source: image_path.into(),
            description: "[API Key 未配置，无法描述图片]".into(),
            content_length: image_data.len(),
            error: Some("API key not configured".into()),
        };
    }

    let url = format!("{}/chat/completions", ai.base_url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let body = json!({
        "model": ai.model,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": "Describe this image concisely in Chinese. Focus on key information." },
                { "type": "image_url", "image_url": { "url": format!("data:{};base64,{}", mime, b64) } }
            ]
        }],
        "max_tokens": 300,
    });

    match client
        .post(&url)
        .header("Authorization", format!("Bearer {}", ai.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            if !resp.status().is_success() {
                let err = resp.text().await.unwrap_or_default();
                return MediaResult {
                    media_type: "image".into(),
                    source: image_path.into(),
                    description: format!("[图片描述失败]"),
                    content_length: image_data.len(),
                    error: Some(format!("API error: {}", &err[..err.len().min(200)])),
                };
            }
            match resp.json::<Value>().await {
                Ok(data) => {
                    let desc = data["choices"][0]["message"]["content"]
                        .as_str()
                        .unwrap_or("[无描述]")
                        .to_string();
                    MediaResult {
                        media_type: "image".into(),
                        source: image_path.into(),
                        description: desc,
                        content_length: image_data.len(),
                        error: None,
                    }
                }
                Err(e) => MediaResult {
                    media_type: "image".into(),
                    source: image_path.into(),
                    description: String::new(),
                    content_length: image_data.len(),
                    error: Some(format!("Parse: {}", e)),
                },
            }
        }
        Err(e) => MediaResult {
            media_type: "image".into(),
            source: image_path.into(),
            description: String::new(),
            content_length: image_data.len(),
            error: Some(format!("Request: {}", e)),
        },
    }
}

// ============================================================================
// Audio Transcription (Whisper API)
// ============================================================================

/// Transcribe audio using OpenAI Whisper API.
pub async fn transcribe_audio(audio_path: &str) -> MediaResult {
    let path = Path::new(audio_path);
    if !path.exists() {
        return MediaResult {
            media_type: "audio".into(),
            source: audio_path.into(),
            description: String::new(),
            content_length: 0,
            error: Some("Audio file not found".into()),
        };
    }

    let audio_data = match std::fs::read(path) {
        Ok(data) => data,
        Err(e) => {
            return MediaResult {
                media_type: "audio".into(),
                source: audio_path.into(),
                description: String::new(),
                content_length: 0,
                error: Some(format!("Read audio: {}", e)),
            };
        }
    };

    let config = match crate::modules::config::load_app_config() {
        Ok(c) => c,
        Err(e) => {
            return MediaResult {
                media_type: "audio".into(),
                source: audio_path.into(),
                description: String::new(),
                content_length: audio_data.len(),
                error: Some(format!("Config: {}", e)),
            };
        }
    };

    let ai = &config.ai_config;
    if ai.api_key.is_empty() {
        return MediaResult {
            media_type: "audio".into(),
            source: audio_path.into(),
            description: "[API Key 未配置，无法转录音频]".into(),
            content_length: audio_data.len(),
            error: Some("API key not configured".into()),
        };
    }

    let url = format!("{}/audio/transcriptions", ai.base_url.trim_end_matches('/'));
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("audio.mp3");

    let file_part = reqwest::multipart::Part::bytes(audio_data.clone())
        .file_name(filename.to_string())
        .mime_str("audio/mpeg")
        .unwrap_or_else(|_| reqwest::multipart::Part::bytes(audio_data.clone()));

    let form = reqwest::multipart::Form::new()
        .part("file", file_part)
        .text("model", "whisper-1")
        .text("language", "zh");

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    match client
        .post(&url)
        .header("Authorization", format!("Bearer {}", ai.api_key))
        .multipart(form)
        .send()
        .await
    {
        Ok(resp) => {
            if !resp.status().is_success() {
                let err = resp.text().await.unwrap_or_default();
                return MediaResult {
                    media_type: "audio".into(),
                    source: audio_path.into(),
                    description: "[音频转录失败]".into(),
                    content_length: audio_data.len(),
                    error: Some(format!("API error: {}", &err[..err.len().min(200)])),
                };
            }
            match resp.json::<Value>().await {
                Ok(data) => {
                    let text = data["text"].as_str().unwrap_or("").to_string();
                    MediaResult {
                        media_type: "audio".into(),
                        source: audio_path.into(),
                        description: text,
                        content_length: audio_data.len(),
                        error: None,
                    }
                }
                Err(e) => MediaResult {
                    media_type: "audio".into(),
                    source: audio_path.into(),
                    description: String::new(),
                    content_length: audio_data.len(),
                    error: Some(format!("Parse: {}", e)),
                },
            }
        }
        Err(e) => MediaResult {
            media_type: "audio".into(),
            source: audio_path.into(),
            description: String::new(),
            content_length: audio_data.len(),
            error: Some(format!("Request: {}", e)),
        },
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn media_detect_mime(path: String) -> Result<String, String> {
    Ok(detect_mime(&path))
}

#[tauri::command]
pub async fn media_extract_file(path: String, max_chars: Option<usize>) -> Result<MediaResult, String> {
    Ok(extract_file_content(&path, max_chars.unwrap_or(10000)))
}

#[tauri::command]
pub async fn media_describe_image(path: String) -> Result<MediaResult, String> {
    Ok(describe_image(&path).await)
}

#[tauri::command]
pub async fn media_transcribe_audio(path: String) -> Result<MediaResult, String> {
    Ok(transcribe_audio(&path).await)
}
