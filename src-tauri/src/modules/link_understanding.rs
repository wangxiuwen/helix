//! Link Understanding — URL detection, content fetching, and summarization.
//!
//! Ported from OpenClaw `src/link-understanding/`: detects URLs in user messages,
//! fetches their content, strips HTML, and injects summaries into the agent context.

use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::info;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkResult {
    pub url: String,
    pub title: Option<String>,
    pub content: String,
    pub content_length: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkUnderstandingResult {
    pub urls: Vec<String>,
    pub results: Vec<LinkResult>,
    pub prompt_injection: String,
}

// ============================================================================
// URL Detection
// ============================================================================

/// Extract URLs from text using regex.
pub fn detect_urls(text: &str) -> Vec<String> {
    let url_regex = Regex::new(
        r#"https?://[^\s<>\[\]\(\)\{\}\|\\^`'""]+"#
    ).unwrap();

    url_regex
        .find_iter(text)
        .map(|m| {
            let url = m.as_str();
            // Trim trailing punctuation
            url.trim_end_matches(|c: char| matches!(c, '.' | ',' | ';' | ':' | ')' | ']' | '>'))
                .to_string()
        })
        .collect()
}

// ============================================================================
// Content Fetching & Processing
// ============================================================================

/// Fetch URL content and extract readable text.
pub async fn fetch_and_summarize(url: &str, max_chars: usize) -> LinkResult {
    let client = match reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 Helix/1.0")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return LinkResult {
                url: url.to_string(),
                title: None,
                content: String::new(),
                content_length: 0,
                error: Some(format!("Failed to create HTTP client: {}", e)),
            };
        }
    };

    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            return LinkResult {
                url: url.to_string(),
                title: None,
                content: String::new(),
                content_length: 0,
                error: Some(format!("Fetch failed: {}", e)),
            };
        }
    };

    let status = resp.status();
    if !status.is_success() {
        return LinkResult {
            url: url.to_string(),
            title: None,
            content: String::new(),
            content_length: 0,
            error: Some(format!("HTTP {}", status)),
        };
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let body = match resp.text().await {
        Ok(b) => b,
        Err(e) => {
            return LinkResult {
                url: url.to_string(),
                title: None,
                content: String::new(),
                content_length: 0,
                error: Some(format!("Read body failed: {}", e)),
            };
        }
    };

    // Extract title from HTML
    let title = if content_type.contains("html") {
        extract_html_title(&body)
    } else {
        None
    };

    // Strip HTML and extract readable text
    let text = if content_type.contains("html") {
        strip_html(&body)
    } else {
        body.clone()
    };

    // Truncate
    let total_len = text.len();
    let truncated = if text.len() > max_chars {
        format!("{}...\n[截断，共 {} 字符]", &text[..max_chars], total_len)
    } else {
        text
    };

    LinkResult {
        url: url.to_string(),
        title,
        content: truncated,
        content_length: total_len,
        error: None,
    }
}

/// Extract <title> from HTML.
fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let after_tag = lower[start..].find('>')?;
    let content_start = start + after_tag + 1;
    let end = lower[content_start..].find("</title>")?;
    let title = html[content_start..content_start + end].trim();
    if title.is_empty() {
        None
    } else {
        Some(html_decode(title))
    }
}

/// Simple HTML entity decoder.
fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
}

/// Strip HTML tags and extract readable text.
fn strip_html(html: &str) -> String {
    // Remove script and style blocks
    let no_script = Regex::new(r"(?is)<(script|style|noscript)[^>]*>.*?</\1>").unwrap();
    let cleaned = no_script.replace_all(html, "");

    // Remove HTML tags
    let no_tags = Regex::new(r"<[^>]+>").unwrap();
    let text = no_tags.replace_all(&cleaned, " ");

    // Decode entities
    let decoded = html_decode(&text);

    // Normalize whitespace
    let normalized = Regex::new(r"\s+").unwrap();
    let result = normalized.replace_all(&decoded, " ");

    result.trim().to_string()
}

// ============================================================================
// Public API
// ============================================================================

/// Process a message: detect URLs, fetch content, build context injection.
pub async fn process_message_links(text: &str, max_urls: usize, max_chars_per_url: usize) -> LinkUnderstandingResult {
    let urls = detect_urls(text);

    if urls.is_empty() {
        return LinkUnderstandingResult {
            urls: vec![],
            results: vec![],
            prompt_injection: String::new(),
        };
    }

    let urls_to_fetch: Vec<String> = urls.into_iter().take(max_urls).collect();
    let mut results = Vec::new();

    for url in &urls_to_fetch {
        info!("Fetching link: {}", url);
        let result = fetch_and_summarize(url, max_chars_per_url).await;
        results.push(result);
    }

    // Build prompt injection
    let mut injection = String::new();
    let successful: Vec<&LinkResult> = results.iter().filter(|r| r.error.is_none()).collect();

    if !successful.is_empty() {
        injection.push_str("\n\n## Link Content\n\nThe following URLs were found in the message and their content has been fetched:\n\n");
        for result in &successful {
            let title = result.title.as_deref().unwrap_or("(no title)");
            injection.push_str(&format!(
                "### [{}]({})\n\n{}\n\n---\n\n",
                title, result.url, result.content
            ));
        }
    }

    LinkUnderstandingResult {
        urls: urls_to_fetch,
        results,
        prompt_injection: injection,
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn link_fetch(url: String, max_chars: Option<usize>) -> Result<LinkResult, String> {
    Ok(fetch_and_summarize(&url, max_chars.unwrap_or(5000)).await)
}

#[tauri::command]
pub async fn link_detect(text: String) -> Result<Vec<String>, String> {
    Ok(detect_urls(&text))
}

#[tauri::command]
pub async fn link_process(text: String, max_urls: Option<usize>, max_chars: Option<usize>) -> Result<LinkUnderstandingResult, String> {
    Ok(process_message_links(&text, max_urls.unwrap_or(3), max_chars.unwrap_or(5000)).await)
}
