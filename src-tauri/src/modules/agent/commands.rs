//! Command System — Rich `/command` registry with argument parsing,
//! text alias detection, and skill-contributed commands.
//!
//! Ported from OpenClaw `src/auto-reply/commands-registry.ts`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{skills};
use crate::modules::database;
use crate::modules::config::{load_app_config, save_app_config};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandArgDef {
    /// Argument name
    pub name: String,
    /// Description
    pub description: String,
    /// Whether this argument is required
    #[serde(default)]
    pub required: bool,
    /// Type hint: "string", "number", "choice"
    #[serde(default = "default_arg_type")]
    pub arg_type: String,
    /// Available choices (for choice type)
    #[serde(default)]
    pub choices: Vec<String>,
    /// Default value
    #[serde(default)]
    pub default: Option<String>,
}

fn default_arg_type() -> String {
    "string".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDef {
    /// Command key (e.g. "reset", "model")
    pub key: String,
    /// Display name
    pub name: String,
    /// Description
    pub description: String,
    /// Category for grouping
    pub category: String,
    /// Aliases (e.g. ["clear"] for "reset")
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Argument definitions
    #[serde(default)]
    pub args: Vec<CommandArgDef>,
    /// Whether this is a built-in command
    #[serde(default = "default_true")]
    pub builtin: bool,
    /// Whether this command is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Authorization required
    #[serde(default)]
    pub auth_required: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub key: String,
    pub raw_args: String,
    pub positional_args: Vec<String>,
    pub named_args: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum ParsedInput {
    Command(ParsedCommand),
    Message(String),
}

/// Text alias for natural language command detection
#[derive(Debug, Clone)]
pub struct TextAlias {
    pub pattern: String,
    pub canonical_command: String,
    pub accepts_args: bool,
}

// ============================================================================
// Built-in Command Registry
// ============================================================================

pub fn get_builtin_commands() -> Vec<CommandDef> {
    vec![
        CommandDef {
            key: "reset".into(),
            name: "重置对话".into(),
            description: "清除当前对话历史".into(),
            category: "session".into(),
            aliases: vec!["clear".into()],
            args: vec![],
            builtin: true,
            enabled: true,
            auth_required: false,
        },
        CommandDef {
            key: "status".into(),
            name: "状态".into(),
            description: "显示 Agent 状态信息".into(),
            category: "info".into(),
            aliases: vec!["info".into()],
            args: vec![],
            builtin: true,
            enabled: true,
            auth_required: false,
        },
        CommandDef {
            key: "model".into(),
            name: "模型切换".into(),
            description: "显示或切换 AI 模型".into(),
            category: "config".into(),
            aliases: vec![],
            args: vec![CommandArgDef {
                name: "model_name".into(),
                description: "模型名称".into(),
                required: false,
                arg_type: "string".into(),
                choices: vec![],
                default: None,
            }],
            builtin: true,
            enabled: true,
            auth_required: false,
        },
        CommandDef {
            key: "help".into(),
            name: "帮助".into(),
            description: "显示可用命令列表".into(),
            category: "info".into(),
            aliases: vec!["?".into()],
            args: vec![],
            builtin: true,
            enabled: true,
            auth_required: false,
        },
        CommandDef {
            key: "memo".into(),
            name: "备忘录".into(),
            description: "存储或查询记忆".into(),
            category: "memory".into(),
            aliases: vec!["memory".into(), "note".into()],
            args: vec![
                CommandArgDef {
                    name: "action".into(),
                    description: "操作: save/search/list".into(),
                    required: true,
                    arg_type: "choice".into(),
                    choices: vec!["save".into(), "search".into(), "list".into()],
                    default: Some("search".into()),
                },
                CommandArgDef {
                    name: "content".into(),
                    description: "内容".into(),
                    required: false,
                    arg_type: "string".into(),
                    choices: vec![],
                    default: None,
                },
            ],
            builtin: true,
            enabled: true,
            auth_required: false,
        },
        CommandDef {
            key: "search".into(),
            name: "搜索".into(),
            description: "搜索网络".into(),
            category: "tools".into(),
            aliases: vec!["google".into(), "web".into()],
            args: vec![CommandArgDef {
                name: "query".into(),
                description: "搜索关键词".into(),
                required: true,
                arg_type: "string".into(),
                choices: vec![],
                default: None,
            }],
            builtin: true,
            enabled: true,
            auth_required: false,
        },
        CommandDef {
            key: "link".into(),
            name: "链接解析".into(),
            description: "抓取并总结 URL 内容".into(),
            category: "tools".into(),
            aliases: vec!["url".into(), "fetch".into()],
            args: vec![CommandArgDef {
                name: "url".into(),
                description: "URL 地址".into(),
                required: true,
                arg_type: "string".into(),
                choices: vec![],
                default: None,
            }],
            builtin: true,
            enabled: true,
            auth_required: false,
        },
        CommandDef {
            key: "audit".into(),
            name: "安全审计".into(),
            description: "运行安全审计检查".into(),
            category: "security".into(),
            aliases: vec!["security".into()],
            args: vec![],
            builtin: true,
            enabled: true,
            auth_required: false,
        },
        CommandDef {
            key: "skills".into(),
            name: "技能".into(),
            description: "列出已启用的技能".into(),
            category: "info".into(),
            aliases: vec![],
            args: vec![],
            builtin: true,
            enabled: true,
            auth_required: false,
        },
        CommandDef {
            key: "cron".into(),
            name: "定时任务".into(),
            description: "列出活跃的定时任务".into(),
            category: "info".into(),
            aliases: vec!["tasks".into()],
            args: vec![],
            builtin: true,
            enabled: true,
            auth_required: false,
        },
    ]
}

/// Get text aliases for natural language command detection.
fn get_text_aliases() -> Vec<TextAlias> {
    vec![
        TextAlias { pattern: "搜索".into(), canonical_command: "search".into(), accepts_args: true },
        TextAlias { pattern: "查找".into(), canonical_command: "search".into(), accepts_args: true },
        TextAlias { pattern: "搜一下".into(), canonical_command: "search".into(), accepts_args: true },
        TextAlias { pattern: "帮我搜".into(), canonical_command: "search".into(), accepts_args: true },
        TextAlias { pattern: "抓取".into(), canonical_command: "link".into(), accepts_args: true },
        TextAlias { pattern: "打开链接".into(), canonical_command: "link".into(), accepts_args: true },
        TextAlias { pattern: "备忘".into(), canonical_command: "memo".into(), accepts_args: true },
        TextAlias { pattern: "记住".into(), canonical_command: "memo".into(), accepts_args: true },
        TextAlias { pattern: "重置".into(), canonical_command: "reset".into(), accepts_args: false },
        TextAlias { pattern: "清除对话".into(), canonical_command: "reset".into(), accepts_args: false },
        TextAlias { pattern: "状态".into(), canonical_command: "status".into(), accepts_args: false },
    ]
}

// ============================================================================
// Command Parsing
// ============================================================================

/// Parse user input into a command or plain message.
pub fn parse_input(input: &str) -> ParsedInput {
    let trimmed = input.trim();

    // 1. Slash command: /cmd args...
    if trimmed.starts_with('/') {
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        let raw_key = parts[0][1..].to_lowercase(); // strip leading /
        let raw_args = if parts.len() > 1 { parts[1].to_string() } else { String::new() };

        // Resolve aliases
        let key = resolve_alias(&raw_key);

        let positional_args: Vec<String> = if raw_args.is_empty() {
            vec![]
        } else {
            raw_args.split_whitespace().map(|s| s.to_string()).collect()
        };

        return ParsedInput::Command(ParsedCommand {
            key,
            raw_args: raw_args.clone(),
            positional_args,
            named_args: HashMap::new(),
        });
    }

    // 2. Text alias detection
    if let Some(parsed) = try_text_alias(trimmed) {
        return ParsedInput::Command(parsed);
    }

    // 3. Plain message
    ParsedInput::Message(trimmed.to_string())
}

/// Resolve a command key through aliases.
fn resolve_alias(key: &str) -> String {
    let commands = get_builtin_commands();
    for cmd in &commands {
        if cmd.key == key {
            return cmd.key.clone();
        }
        for alias in &cmd.aliases {
            if alias == key {
                return cmd.key.clone();
            }
        }
    }
    key.to_string()
}

/// Try to match input against text aliases.
fn try_text_alias(input: &str) -> Option<ParsedCommand> {
    let aliases = get_text_aliases();
    for alias in &aliases {
        if input.starts_with(&alias.pattern) {
            let rest = input[alias.pattern.len()..].trim().to_string();
            if !alias.accepts_args && !rest.is_empty() {
                continue; // alias doesn't accept args but user provided some
            }
            let positional_args: Vec<String> = if rest.is_empty() {
                vec![]
            } else {
                rest.split_whitespace().map(|s| s.to_string()).collect()
            };
            return Some(ParsedCommand {
                key: alias.canonical_command.clone(),
                raw_args: rest.clone(),
                positional_args,
                named_args: HashMap::new(),
            });
        }
    }
    None
}

// ============================================================================
// Command Execution
// ============================================================================

/// Execute a parsed command. Returns Some(response) if handled, None if unknown.
pub fn execute_command(cmd: &ParsedCommand, account_id: &str) -> Option<String> {
    match cmd.key.as_str() {
        "reset" | "clear" => {
            if let Err(e) = database::clear_messages(account_id) {
                Some(format!("❌ 清除失败: {}", e))
            } else {
                Some("✅ 对话历史已清除".to_string())
            }
        }
        "status" => {
            let config = load_app_config().ok();
            let ai = config.as_ref().map(|c| &c.ai_config);
            let skills_list = skills::list_all_skills();
            let enabled_skills = skills_list.iter().filter(|s| s.enabled).count();
            Some(format!(
                "🤖 Helix Agent Status\n\
                 ├ Provider: {}\n\
                 ├ Model: {}\n\
                 ├ Max Tokens: {}\n\
                 ├ Skills: {}/{} enabled\n\
                 └ Time: {}",
                ai.map(|a| a.provider.as_str()).unwrap_or("unknown"),
                ai.map(|a| a.model.as_str()).unwrap_or("unknown"),
                ai.map(|a| a.max_tokens).unwrap_or(0),
                enabled_skills,
                skills_list.len(),
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            ))
        }
        "model" => {
            if cmd.positional_args.is_empty() {
                let config = load_app_config().ok();
                Some(format!(
                    "🧠 Current model: {}",
                    config.map(|c| c.ai_config.model).unwrap_or_default()
                ))
            } else {
                let new_model = cmd.raw_args.clone();
                match load_app_config() {
                    Ok(mut config) => {
                        config.ai_config.model = new_model.clone();
                        match save_app_config(&config) {
                            Ok(_) => Some(format!("✅ Model switched to: {}", new_model)),
                            Err(e) => Some(format!("❌ Failed to save: {}", e)),
                        }
                    }
                    Err(e) => Some(format!("❌ Config error: {}", e)),
                }
            }
        }
        "help" => Some(build_help_text()),
        "memo" => handle_memo_command(cmd, account_id),
        "search" => {
            if cmd.raw_args.is_empty() {
                Some("❓ 请提供搜索关键词，例如: /search kubernetes 教程".into())
            } else {
                // Return None to let agent handle via web_search tool
                None
            }
        }
        "link" => {
            if cmd.raw_args.is_empty() {
                Some("❓ 请提供 URL，例如: /link https://example.com".into())
            } else {
                // Return None to let agent handle via web_fetch tool
                None
            }
        }
        "skills" => {
            let skills_list = skills::list_all_skills();
            if skills_list.is_empty() {
                Some("📦 暂无已安装的技能\n将 SKILL.md 放入 ~/.helix/skills/ 目录即可加载".into())
            } else {
                let mut output = format!("📦 已安装技能 ({}):\n", skills_list.len());
                for s in &skills_list {
                    let status = if s.enabled { "✅" } else { "⏸️" };
                    output.push_str(&format!("  {} {} {} — {}\n", status, s.icon, s.name, s.description));
                }
                Some(output)
            }
        }
        "cron" => {
            match crate::modules::cron::list_tasks() {
                Ok(tasks) => {
                    if tasks.is_empty() {
                        Some("⏰ 暂无活跃的定时任务".into())
                    } else {
                        let mut output = format!("⏰ 定时任务 ({}):\n", tasks.len());
                        for t in &tasks {
                            let status_icon = match t.status.as_str() {
                                "active" => "🟢",
                                "paused" => "⏸️",
                                "error" => "🔴",
                                _ => "⚪",
                            };
                            output.push_str(&format!(
                                "  {} {} [{}] {}\n",
                                status_icon,
                                t.name,
                                t.schedule.as_deref().unwrap_or("manual"),
                                t.description
                            ));
                        }
                        Some(output)
                    }
                }
                Err(e) => Some(format!("❌ 加载任务失败: {}", e)),
            }
        }
        "audit" => {
            // Will be handled by security module once Phase 3 is implemented
            Some("🔒 安全审计功能正在开发中...".into())
        }
        _ => None,
    }
}

/// Build the help text showing all available commands.
fn build_help_text() -> String {
    let commands = get_builtin_commands();

    let mut output = String::from("📖 Helix 命令列表:\n\n");

    let mut by_category: HashMap<String, Vec<&CommandDef>> = HashMap::new();
    for cmd in &commands {
        by_category.entry(cmd.category.clone()).or_default().push(cmd);
    }

    let category_labels: HashMap<&str, &str> = [
        ("session", "📝 会话"),
        ("info", "ℹ️ 信息"),
        ("config", "⚙️ 配置"),
        ("memory", "🧠 记忆"),
        ("tools", "🔧 工具"),
        ("security", "🔒 安全"),
    ]
    .iter()
    .cloned()
    .collect();

    let order = ["session", "config", "info", "memory", "tools", "security"];
    for cat in &order {
        if let Some(cmds) = by_category.get(*cat) {
            let label = category_labels.get(cat).unwrap_or(cat);
            output.push_str(&format!("{}:\n", label));
            for cmd in cmds {
                let args_str = if cmd.args.is_empty() {
                    String::new()
                } else {
                    let a: Vec<String> = cmd.args.iter().map(|a| {
                        if a.required {
                            format!("<{}>", a.name)
                        } else {
                            format!("[{}]", a.name)
                        }
                    }).collect();
                    format!(" {}", a.join(" "))
                };
                let aliases = if cmd.aliases.is_empty() {
                    String::new()
                } else {
                    format!(" (别名: {})", cmd.aliases.iter().map(|a| format!("/{}", a)).collect::<Vec<_>>().join(", "))
                };
                output.push_str(&format!(
                    "  /{}{} — {}{}\n",
                    cmd.key, args_str, cmd.description, aliases
                ));
            }
            output.push('\n');
        }
    }

    output.push_str("🔧 Agent 工具:\n");
    output.push_str("  shell_exec, file_read, file_write, file_edit,\n");
    output.push_str("  list_dir, find_files, grep_search,\n");
    output.push_str("  web_fetch, web_search, memory_store, memory_recall,\n");
    output.push_str("  process_list, process_kill, sysinfo\n");

    output
}

/// Handle the /memo command for memory operations.
fn handle_memo_command(cmd: &ParsedCommand, _account_id: &str) -> Option<String> {
    let action = cmd.positional_args.first().map(|s| s.as_str()).unwrap_or("list");
    let content = if cmd.positional_args.len() > 1 {
        cmd.positional_args[1..].join(" ")
    } else {
        String::new()
    };

    match action {
        "save" | "store" => {
            if content.is_empty() {
                return Some("❓ 请提供要保存的内容，例如: /memo save 服务器密码是 xxx".into());
            }
            // Generate a key from first few words
            let key = content.split_whitespace().take(3).collect::<Vec<_>>().join("_");
            match database::memory_store(&key, &content) {
                Ok(_) => Some(format!("✅ 已保存备忘: {}", key)),
                Err(e) => Some(format!("❌ 保存失败: {}", e)),
            }
        }
        "search" | "find" | "recall" => {
            if content.is_empty() {
                return Some("❓ 请提供搜索关键词，例如: /memo search 密码".into());
            }
            match database::memory_recall(&content) {
                Ok(results) => {
                    if results.is_empty() {
                        Some(format!("🔍 未找到匹配: {}", content))
                    } else {
                        let mut output = format!("🔍 搜索结果 ({}):\n", results.len());
                        for (k, v) in &results {
                            output.push_str(&format!("  📌 {}: {}\n", k, v));
                        }
                        Some(output)
                    }
                }
                Err(e) => Some(format!("❌ 搜索失败: {}", e)),
            }
        }
        "list" => {
            match database::memory_recall("") {
                Ok(results) => {
                    if results.is_empty() {
                        Some("📌 暂无保存的备忘录".into())
                    } else {
                        let mut output = format!("📌 备忘录 ({}):\n", results.len());
                        for (k, v) in &results {
                            let preview = if v.len() > 50 { format!("{}...", &v[..50]) } else { v.clone() };
                            output.push_str(&format!("  • {}: {}\n", k, preview));
                        }
                        Some(output)
                    }
                }
                Err(e) => Some(format!("❌ 加载失败: {}", e)),
            }
        }
        _ => Some(format!("❓ 未知操作: {}。支持: save, search, list", action)),
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn commands_list() -> Result<Vec<CommandDef>, String> {
    Ok(get_builtin_commands())
}

#[tauri::command]
pub async fn commands_execute(command: String, args: Option<String>, account_id: String) -> Result<Option<String>, String> {
    let parsed = ParsedCommand {
        key: command.clone(),
        raw_args: args.clone().unwrap_or_default(),
        positional_args: args.map(|a| a.split_whitespace().map(|s| s.to_string()).collect()).unwrap_or_default(),
        named_args: HashMap::new(),
    };
    Ok(execute_command(&parsed, &account_id))
}
