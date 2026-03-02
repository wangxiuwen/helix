//! Unified Context Management — Brain Module
//!
//! Ported from Antigravity's context management system:
//! - Conversation logs with auto-summarization
//! - Knowledge Items (KI) for distilled, reusable knowledge
//! - Smart context injection combining all sources
//! - Backward-compatible GEMINI.md file reading

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationLog {
    pub session_id: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeItem {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub source_sessions: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextIndex {
    pub conversations: Vec<ConversationSummaryEntry>,
    pub knowledge_items: Vec<KnowledgeIndexEntry>,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummaryEntry {
    pub session_id: String,
    pub title: String,
    pub summary: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeIndexEntry {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub tags: Vec<String>,
}

// ============================================================================
// Brain Directory Management
// ============================================================================

/// Get the brain root directory (~/.helix/brain/)
fn brain_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Cannot find home directory".to_string())?;
    let brain = home.join(".helix").join("brain");
    std::fs::create_dir_all(&brain).map_err(|e| format!("create brain dir: {}", e))?;
    Ok(brain)
}

/// Get the conversations directory
fn conversations_dir() -> Result<PathBuf, String> {
    let dir = brain_dir()?.join("conversations");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create conversations dir: {}", e))?;
    Ok(dir)
}

/// Get the knowledge directory
fn knowledge_dir() -> Result<PathBuf, String> {
    let dir = brain_dir()?.join("knowledge");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create knowledge dir: {}", e))?;
    Ok(dir)
}

/// Get the context index path
fn index_path() -> Result<PathBuf, String> {
    Ok(brain_dir()?.join("context_index.json"))
}

/// Initialize the brain directory structure
pub fn init_brain() -> Result<(), String> {
    brain_dir()?;
    conversations_dir()?;
    knowledge_dir()?;

    // Create index if not exists
    let idx_path = index_path()?;
    if !idx_path.exists() {
        let index = ContextIndex {
            conversations: vec![],
            knowledge_items: vec![],
            last_updated: now_rfc3339(),
        };
        write_json(&idx_path, &index)?;
    }

    info!("[brain] Initialized brain directory");
    Ok(())
}

// ============================================================================
// Conversation Logger
// ============================================================================

/// Log a message to the conversation log file (JSONL format)
pub fn log_message(session_id: &str, role: &str, content: &str) -> Result<(), String> {
    let conv_dir = conversations_dir()?.join(session_id);
    std::fs::create_dir_all(&conv_dir).map_err(|e| format!("create conv dir: {}", e))?;

    let log_path = conv_dir.join("log.jsonl");
    let entry = LogEntry {
        timestamp: now_rfc3339(),
        role: role.to_string(),
        content: content.to_string(),
    };

    let line = serde_json::to_string(&entry).map_err(|e| format!("serialize: {}", e))?;

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("open log: {}", e))?;
    writeln!(file, "{}", line).map_err(|e| format!("write log: {}", e))?;

    Ok(())
}

/// Get all messages from a conversation log
pub fn get_conversation_log(session_id: &str) -> Result<Vec<LogEntry>, String> {
    let log_path = conversations_dir()?.join(session_id).join("log.jsonl");
    if !log_path.exists() {
        return Ok(vec![]);
    }

    let content = std::fs::read_to_string(&log_path).map_err(|e| format!("read log: {}", e))?;
    let entries: Vec<LogEntry> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    Ok(entries)
}

/// List all conversation logs with metadata
pub fn list_conversations(limit: usize) -> Result<Vec<ConversationLog>, String> {
    let conv_dir = conversations_dir()?;
    let mut logs = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&conv_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let session_id = entry.file_name().to_string_lossy().to_string();
            let log_path = entry.path().join("log.jsonl");
            let overview_path = entry.path().join("overview.txt");

            if !log_path.exists() {
                continue;
            }

            let content =
                std::fs::read_to_string(&log_path).unwrap_or_default();
            let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
            let message_count = lines.len();

            // Try to parse first and last timestamps
            let created_at = lines
                .first()
                .and_then(|l| serde_json::from_str::<LogEntry>(l).ok())
                .map(|e| e.timestamp)
                .unwrap_or_default();
            let updated_at = lines
                .last()
                .and_then(|l| serde_json::from_str::<LogEntry>(l).ok())
                .map(|e| e.timestamp)
                .unwrap_or_default();

            let summary = std::fs::read_to_string(&overview_path).ok();

            // Extract title from overview or first user message
            let title = summary
                .as_ref()
                .and_then(|s| s.lines().next())
                .map(|s| s.trim_start_matches("# ").to_string())
                .or_else(|| {
                    lines.iter().find_map(|l| {
                        let entry: LogEntry = serde_json::from_str(l).ok()?;
                        if entry.role == "user" {
                            Some(entry.content.chars().take(60).collect::<String>())
                        } else {
                            None
                        }
                    })
                });

            logs.push(ConversationLog {
                session_id,
                title,
                summary,
                created_at,
                updated_at,
                message_count,
            });
        }
    }

    // Sort by updated_at descending
    logs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    logs.truncate(limit);

    Ok(logs)
}

// ============================================================================
// Conversation Summarization
// ============================================================================

/// Generate an overview summary for a conversation using AI
pub async fn summarize_conversation(session_id: &str) -> Result<String, String> {
    let entries = get_conversation_log(session_id)?;
    if entries.is_empty() {
        return Err("No messages to summarize".to_string());
    }

    // Build a condensed transcript for summarization (max ~3000 chars)
    let mut transcript = String::new();
    let mut chars = 0;
    for entry in &entries {
        let prefix = match entry.role.as_str() {
            "user" => "User",
            "assistant" => "AI",
            _ => &entry.role,
        };
        let line = format!("[{}]: {}\n", prefix, &entry.content[..entry.content.len().min(300)]);
        chars += line.len();
        if chars > 3000 {
            transcript.push_str("...(truncated)\n");
            break;
        }
        transcript.push_str(&line);
    }

    // Call AI for summarization
    let config = crate::modules::config::load_app_config()
        .map_err(|e| format!("load config: {}", e))?;
    let ai = &config.ai_config;

    if ai.api_key.is_empty() && ai.provider != "ollama" {
        // Fallback: generate a simple extractive summary
        let first_user_msg = entries
            .iter()
            .find(|e| e.role == "user")
            .map(|e| e.content.chars().take(100).collect::<String>())
            .unwrap_or_else(|| "Conversation".to_string());
        let summary = format!(
            "# {}\n\n对话包含 {} 条消息。\n主题: {}",
            first_user_msg.chars().take(60).collect::<String>(),
            entries.len(),
            first_user_msg
        );
        save_overview(session_id, &summary)?;
        return Ok(summary);
    }

    let provider = super::providers::resolve_provider_config(
        &ai.model,
        Some(&ai.base_url),
        Some(&ai.api_key),
        None,
    );

    let prompt = format!(
        "请用中文总结以下对话，生成一个简洁的概要（3-5句话）。\n\
         第一行应该是一个简短的标题（不超过20个字），用 # 开头。\n\
         后面是对话的主要内容、关键决策和结论。\n\n\
         对话内容:\n{}",
        transcript
    );

    let body = super::providers::build_openai_request(
        &ai.model,
        &[json!({"role": "user", "content": prompt})],
        None,
        500,
        false,
    );

    let result = super::streaming::complete_simple(&provider, &body).await?;
    let summary = if result.content.is_empty() {
        format!("# Conversation\n\n{} messages exchanged.", entries.len())
    } else {
        result.content
    };

    save_overview(session_id, &summary)?;
    update_conversation_in_index(session_id, &summary)?;

    info!(
        "[brain] Summarized conversation '{}' ({} messages)",
        session_id,
        entries.len()
    );

    Ok(summary)
}

/// Save overview text to conversation directory
fn save_overview(session_id: &str, summary: &str) -> Result<(), String> {
    let overview_path = conversations_dir()?.join(session_id).join("overview.txt");
    std::fs::write(&overview_path, summary).map_err(|e| format!("write overview: {}", e))?;
    Ok(())
}

/// Update a conversation entry in the context index
fn update_conversation_in_index(session_id: &str, summary: &str) -> Result<(), String> {
    let idx_path = index_path()?;
    let mut index = load_index()?;

    // Extract title from first line
    let title = summary
        .lines()
        .next()
        .unwrap_or("Conversation")
        .trim_start_matches("# ")
        .to_string();

    // Update or insert
    if let Some(entry) = index
        .conversations
        .iter_mut()
        .find(|c| c.session_id == session_id)
    {
        entry.title = title;
        entry.summary = summary.to_string();
        entry.updated_at = now_rfc3339();
    } else {
        index.conversations.push(ConversationSummaryEntry {
            session_id: session_id.to_string(),
            title,
            summary: summary.to_string(),
            updated_at: now_rfc3339(),
        });
    }

    // Keep only last 50 conversations in index
    index
        .conversations
        .sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    index.conversations.truncate(50);
    index.last_updated = now_rfc3339();

    write_json(&idx_path, &index)?;
    Ok(())
}

// ============================================================================
// Knowledge Items (KI)
// ============================================================================

/// Create a new knowledge item
pub fn create_knowledge_item(
    title: &str,
    content: &str,
    tags: &[String],
    source_sessions: &[String],
) -> Result<KnowledgeItem, String> {
    let id = format!("ki_{}", &uuid_short());
    let ki_dir = knowledge_dir()?.join(&id);
    std::fs::create_dir_all(&ki_dir).map_err(|e| format!("create KI dir: {}", e))?;

    let now = now_rfc3339();
    let ki = KnowledgeItem {
        id: id.clone(),
        title: title.to_string(),
        summary: content.lines().take(3).collect::<Vec<_>>().join(" "),
        tags: tags.to_vec(),
        source_sessions: source_sessions.to_vec(),
        created_at: now.clone(),
        updated_at: now,
    };

    // Save metadata
    let meta_path = ki_dir.join("metadata.json");
    write_json(&meta_path, &ki)?;

    // Save content
    let content_path = ki_dir.join("content.md");
    std::fs::write(&content_path, content).map_err(|e| format!("write content: {}", e))?;

    // Update index
    update_knowledge_in_index(&ki)?;

    info!("[brain] Created KI '{}': {}", id, title);
    Ok(ki)
}

/// Get a knowledge item by ID
pub fn get_knowledge_item(id: &str) -> Result<(KnowledgeItem, String), String> {
    let ki_dir = knowledge_dir()?.join(id);
    let meta_path = ki_dir.join("metadata.json");
    let content_path = ki_dir.join("content.md");

    let meta: KnowledgeItem = read_json(&meta_path)?;
    let content =
        std::fs::read_to_string(&content_path).map_err(|e| format!("read content: {}", e))?;

    Ok((meta, content))
}

/// List all knowledge items
pub fn list_knowledge_items() -> Result<Vec<KnowledgeItem>, String> {
    let ki_dir = knowledge_dir()?;
    let mut items = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&ki_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let meta_path = entry.path().join("metadata.json");
            if let Ok(ki) = read_json::<KnowledgeItem>(&meta_path) {
                items.push(ki);
            }
        }
    }

    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(items)
}

/// Search knowledge items by query (title, tags, summary)
pub fn search_knowledge(query: &str, limit: usize) -> Result<Vec<KnowledgeItem>, String> {
    let all = list_knowledge_items()?;
    let query_lower = query.to_lowercase();

    let mut matches: Vec<(usize, &KnowledgeItem)> = all
        .iter()
        .filter_map(|ki| {
            let mut score = 0usize;
            if ki.title.to_lowercase().contains(&query_lower) {
                score += 10;
            }
            if ki.summary.to_lowercase().contains(&query_lower) {
                score += 5;
            }
            for tag in &ki.tags {
                if tag.to_lowercase().contains(&query_lower) {
                    score += 8;
                }
            }
            if score > 0 {
                Some((score, ki))
            } else {
                None
            }
        })
        .collect();

    matches.sort_by(|a, b| b.0.cmp(&a.0));
    let results: Vec<KnowledgeItem> = matches. iter().take(limit).map(|(_, ki)| (*ki).clone()).collect();

    Ok(results)
}

/// Delete a knowledge item
pub fn delete_knowledge_item(id: &str) -> Result<(), String> {
    let ki_dir = knowledge_dir()?.join(id);
    if ki_dir.exists() {
        std::fs::remove_dir_all(&ki_dir).map_err(|e| format!("delete KI: {}", e))?;
    }

    // Remove from index
    let idx_path = index_path()?;
    let mut index = load_index()?;
    index.knowledge_items.retain(|ki| ki.id != id);
    index.last_updated = now_rfc3339();
    write_json(&idx_path, &index)?;

    info!("[brain] Deleted KI '{}'", id);
    Ok(())
}

/// Update KI in the context index
fn update_knowledge_in_index(ki: &KnowledgeItem) -> Result<(), String> {
    let idx_path = index_path()?;
    let mut index = load_index()?;

    if let Some(entry) = index.knowledge_items.iter_mut().find(|k| k.id == ki.id) {
        entry.title = ki.title.clone();
        entry.summary = ki.summary.clone();
        entry.tags = ki.tags.clone();
    } else {
        index.knowledge_items.push(KnowledgeIndexEntry {
            id: ki.id.clone(),
            title: ki.title.clone(),
            summary: ki.summary.clone(),
            tags: ki.tags.clone(),
        });
    }

    index.last_updated = now_rfc3339();
    write_json(&idx_path, &index)?;
    Ok(())
}

// ============================================================================
// Unified Context Injection
// ============================================================================

/// Get full context for a session: GEMINI.md + KI summaries + recent conversations
///
/// This is the MAIN entry point that replaces the old `get_antigravity_context()`.
/// It combines all context sources into a single coherent prompt section.
pub fn get_full_context(session_id: Option<&str>, workspace: Option<String>) -> String {
    let mut sections = Vec::new();

    // 1. GEMINI.md (backward compatible)
    let gemini_ctx = load_gemini_context(workspace);
    if !gemini_ctx.is_empty() {
        sections.push(gemini_ctx);
    }

    // 2. Knowledge Items summaries
    if let Ok(index) = load_index() {
        if !index.knowledge_items.is_empty() {
            let ki_summary: Vec<String> = index
                .knowledge_items
                .iter()
                .take(20) // Cap at 20 KIs
                .map(|ki| {
                    let tags = if ki.tags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", ki.tags.join(", "))
                    };
                    format!("- **{}**{}: {}", ki.title, tags, ki.summary)
                })
                .collect();

            sections.push(format!(
                "<knowledge_items>\n\
                 以下是从之前对话中提取的知识条目。如果和当前话题相关，可以参考：\n\n{}\n\
                 </knowledge_items>",
                ki_summary.join("\n")
            ));
        }

        // 3. Recent conversation summaries (exclude current session)
        let recent: Vec<&ConversationSummaryEntry> = index
            .conversations
            .iter()
            .filter(|c| session_id.map_or(true, |sid| c.session_id != sid))
            .take(5)
            .collect();

        if !recent.is_empty() {
            let conv_summary: Vec<String> = recent
                .iter()
                .map(|c| format!("- [{}] {}", c.title, c.summary.lines().take(2).collect::<Vec<_>>().join(" ")))
                .collect();

            sections.push(format!(
                "<recent_conversations>\n\
                 最近的对话摘要供参考：\n\n{}\n\
                 </recent_conversations>",
                conv_summary.join("\n")
            ));
        }
    }

    if sections.is_empty() {
        return String::new();
    }

    format!(
        "<persistent_context>\n{}\n</persistent_context>\n",
        sections.join("\n\n")
    )
}

/// Backward-compatible: load GEMINI.md context (same logic as before)
fn load_gemini_context(workspace: Option<String>) -> String {
    let mut rules = String::new();

    // 1. Global context (~/.gemini/GEMINI.md)
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();

    if !home.is_empty() {
        let global_gemini = Path::new(&home).join(".gemini").join("GEMINI.md");
        if let Ok(content) = std::fs::read_to_string(&global_gemini) {
            rules.push_str("<MEMORY[user_global]>\n");
            rules.push_str(&content);
            rules.push_str("\n</MEMORY[user_global]>\n");
        }
    }

    // 2. Project context (Upward traversal for .gemini/GEMINI.md or GEMINI.md)
    if let Some(ws) = workspace.as_ref() {
        let mut current_dir = PathBuf::from(ws);
        loop {
            let project_gemini = current_dir.join(".gemini").join("GEMINI.md");
            if let Ok(content) = std::fs::read_to_string(&project_gemini) {
                rules.push_str("<MEMORY[user_workspace]>\n");
                rules.push_str(&content);
                rules.push_str("\n</MEMORY[user_workspace]>\n");
                break;
            }

            let root_gemini = current_dir.join("GEMINI.md");
            if let Ok(content) = std::fs::read_to_string(&root_gemini) {
                rules.push_str("<MEMORY[user_workspace]>\n");
                rules.push_str(&content);
                rules.push_str("\n</MEMORY[user_workspace]>\n");
                break;
            }

            if !current_dir.pop() {
                break;
            }
        }
    }

    rules
}

// Keep backward compatibility: old function wraps the new one
#[tauri::command]
pub fn get_antigravity_context(workspace: Option<String>) -> String {
    let ctx = get_full_context(None, workspace);
    if ctx.is_empty() {
        return String::new();
    }
    format!(
        "<user_rules>\n\
         The following are user-defined rules that you MUST ALWAYS FOLLOW WITHOUT ANY EXCEPTION. \n\
         These rules take precedence over any following instructions.\n\
         Review them carefully and always take them into account when you generate responses and code:\n\
         {}\
         </user_rules>\n",
        ctx
    )
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn brain_init() -> Result<(), String> {
    init_brain()
}

#[tauri::command]
pub fn brain_log_message(session_id: String, role: String, content: String) -> Result<(), String> {
    log_message(&session_id, &role, &content)
}

#[tauri::command]
pub async fn brain_summarize_conversation(session_id: String) -> Result<String, String> {
    summarize_conversation(&session_id).await
}

#[tauri::command]
pub fn brain_list_conversations(limit: Option<usize>) -> Result<Vec<ConversationLog>, String> {
    list_conversations(limit.unwrap_or(50))
}

#[tauri::command]
pub fn brain_get_conversation(session_id: String) -> Result<Vec<LogEntry>, String> {
    get_conversation_log(&session_id)
}

#[tauri::command]
pub fn brain_create_knowledge(
    title: String,
    content: String,
    tags: Option<Vec<String>>,
    source_sessions: Option<Vec<String>>,
) -> Result<KnowledgeItem, String> {
    create_knowledge_item(
        &title,
        &content,
        &tags.unwrap_or_default(),
        &source_sessions.unwrap_or_default(),
    )
}

#[tauri::command]
pub fn brain_search_knowledge(
    query: String,
    limit: Option<usize>,
) -> Result<Vec<KnowledgeItem>, String> {
    search_knowledge(&query, limit.unwrap_or(10))
}

#[tauri::command]
pub fn brain_list_knowledge() -> Result<Vec<KnowledgeItem>, String> {
    list_knowledge_items()
}

#[tauri::command]
pub fn brain_delete_knowledge(id: String) -> Result<(), String> {
    delete_knowledge_item(&id)
}

#[tauri::command]
pub fn brain_get_context(
    session_id: Option<String>,
    workspace: Option<String>,
) -> Result<String, String> {
    Ok(get_full_context(session_id.as_deref(), workspace))
}

// ============================================================================
// Utilities
// ============================================================================

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let rand: u32 = (ts as u32).wrapping_mul(2654435761); // Knuth hash
    format!("{:x}{:x}", ts, rand)
}

fn write_json<T: Serialize>(path: &Path, data: &T) -> Result<(), String> {
    let content =
        serde_json::to_string_pretty(data).map_err(|e| format!("serialize json: {}", e))?;
    std::fs::write(path, content).map_err(|e| format!("write json: {}", e))?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("read json: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("parse json: {}", e))
}

fn load_index() -> Result<ContextIndex, String> {
    let idx_path = index_path()?;
    if idx_path.exists() {
        read_json(&idx_path)
    } else {
        Ok(ContextIndex {
            conversations: vec![],
            knowledge_items: vec![],
            last_updated: now_rfc3339(),
        })
    }
}
