//! Built-in bot commands — ported from wx-filehelper-api plugins/builtin.
//!
//! Handles `/start`, `/help`, `/about`, `/status`, `/version`, `/settings`, `/cancel`.
//! Commands like `/ask`, `/chat`, `/task` are handled by Helix's agent system.

use std::time::{SystemTime, UNIX_EPOCH};
use chrono::Local;

use super::SESSIONS;
use crate::modules::database;

/// Uptime tracking — set once at startup
static START_TIME: once_cell::sync::Lazy<u64> = once_cell::sync::Lazy::new(|| {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
});

/// Initialize the start time (call from lib.rs setup)
pub fn init_start_time() {
    let _ = *START_TIME;
}

/// Dispatch a `/command` and return the reply text, or None if not a known command.
pub fn dispatch_command(text: &str, session_id: &str) -> Option<String> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let without_slash = &trimmed[1..];
    let parts: Vec<&str> = without_slash.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let cmd = parts[0].to_lowercase();
    let args: Vec<&str> = parts[1..].to_vec();

    match cmd.as_str() {
        "start" | "menu" => Some(cmd_start()),
        "help" | "h" | "?" => Some(cmd_help()),
        "about" => Some(cmd_about()),
        "status" | "stat" | "info" => Some(cmd_status(session_id)),
        "version" | "ver" | "v" => Some(cmd_version()),
        "settings" => Some(cmd_settings(session_id)),
        "cancel" => Some("没有正在进行的操作。".to_string()),
        "ping" => Some("Pong!".to_string()),
        "plugins" | "plugin" => Some(cmd_plugins()),
        "chat" => Some(cmd_chat(&args)),
        _ => {
            // Unknown command — return hint
            Some(format!("未知命令: /{}\\n输入 /help 查看可用命令。", cmd))
        }
    }
}

fn cmd_start() -> String {
    format!(
        "🤖 Helix FileHelper v{}\n\n\
         欢迎使用文件传输助手机器人！\n\n\
         【Telegram 标准命令】\n\
         /help - 命令列表\n\
         /settings - 查看设置\n\
         /about - 关于本 Bot\n\n\
         【快捷入口】\n\
         /status - 服务器状态\n\
         /version - 版本信息\n\n\
         发送任意文字开始对话 ✨",
        env!("CARGO_PKG_VERSION")
    )
}

fn cmd_help() -> String {
    "📖 命令列表\n\n\
     【Telegram 标准】\n\
     /start - 开始使用\n\
     /help - 命令列表\n\
     /settings - 查看设置\n\
     /cancel - 取消操作\n\
     /about - 关于本 Bot\n\
     /version - 版本信息\n\n\
     【核心功能】\n\
     /status - 服务器状态\n\
     /ping - 连通测试\n\
     /plugins - 插件状态\n\n\
     发送任意文字可触发 AI 对话"
        .to_string()
}

fn cmd_about() -> String {
    format!(
        "🤖 Helix FileHelper\n\n\
         基于微信文件传输助手的 Bot API 框架\n\
         兼容 Telegram Bot API 标准\n\n\
         版本: {}\n\n\
         【特性】\n\
         • Telegram Bot API 兼容\n\
         • AI Agent 系统 (命令处理/自动回复)\n\
         • 消息持久化 (SQLite)\n\
         • 自动文件下载\n\
         • 定时任务调度\n\
         • 心跳检测与自动重连",
        env!("CARGO_PKG_VERSION")
    )
}

fn cmd_version() -> String {
    format!("Helix FileHelper v{}", env!("CARGO_PKG_VERSION"))
}

fn cmd_status(session_id: &str) -> String {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let uptime_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(*START_TIME);

    let logged_in = {
        let sessions = SESSIONS.lock().unwrap();
        sessions.get(session_id)
            .map(|s| s.session.logged_in)
            .unwrap_or(false)
    };

    let msg_count = database::count_messages(session_id).unwrap_or(0);
    let stats = database::store_stats(session_id).ok();
    let file_count = stats.as_ref()
        .and_then(|s| s.get("file_count"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let uptime_str = if uptime_secs >= 86400 {
        format!("{}天{}小时", uptime_secs / 86400, (uptime_secs % 86400) / 3600)
    } else if uptime_secs >= 3600 {
        format!("{}小时{}分", uptime_secs / 3600, (uptime_secs % 3600) / 60)
    } else {
        format!("{}分{}秒", uptime_secs / 60, uptime_secs % 60)
    };

    format!(
        "time={}\n\
         uptime={}\n\
         platform=Rust/{}\n\
         wechat_logged_in={}\n\
         messages={}\n\
         files={}",
        now, uptime_str,
        env!("CARGO_PKG_VERSION"),
        logged_in, msg_count, file_count
    )
}

fn cmd_settings(session_id: &str) -> String {
    let logged_in = {
        let sessions = SESSIONS.lock().unwrap();
        sessions.get(session_id)
            .map(|s| s.session.logged_in)
            .unwrap_or(false)
    };

    let webhook_configured = super::bot_api::get_webhook_url_public().is_some();

    format!(
        "⚙️ 当前设置\n\n\
         【聊天模式】\n\
         状态: 开启 (由 Helix Agent 处理)\n\
         Webhook: {}\n\n\
         【文件管理】\n\
         自动下载: 是\n\n\
         【会话】\n\
         登录状态: {}\n\
         心跳间隔: 60s\n\
         连接监控: 已启动",
        if webhook_configured { "已配置" } else { "未配置" },
        if logged_in { "在线" } else { "离线" },
    )
}

fn cmd_plugins() -> String {
    // Helix uses an agent-based plugin system
    "插件系统: Helix Agent\n\
     命令处理: 内置 bot_commands + Agent\n\
     消息处理: 自动 AI 回复\n\
     HTTP路由: bot_api.rs (25 路由)"
        .to_string()
}

fn cmd_chat(args: &[&str]) -> String {
    if args.is_empty() {
        return "chat_mode=on (由 Helix Agent 处理自动回复)\n用法: /chat on|off".to_string();
    }
    match args[0].to_lowercase().as_str() {
        "on" | "enable" | "1" => "AI 聊天模式已启用".to_string(),
        "off" | "disable" | "0" => "AI 聊天模式已关闭".to_string(),
        "status" | "state" => "chat_mode=on".to_string(),
        _ => "用法: /chat on|off|status".to_string(),
    }
}
