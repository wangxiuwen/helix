//! Skills Backend — SKILL.md file loader and system prompt injector.
//!
//! Skills are stored in `~/.helix/skills/<name>/SKILL.md`.
//! Each SKILL.md has YAML frontmatter (name, description, version, author, tags, icon)
//! and a Markdown body that is injected into the agent system prompt.

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use super::config::get_data_dir;

// ============================================================================
// Constants — Built-in skill templates
// ============================================================================

/// Built-in skills that are auto-copied to ~/.helix/skills/ on first run.
const BUILTIN_SKILLS: &[(&str, &str)] = &[
    ("web-search", r#"---
name: web-search
description: 网络搜索和信息检索技能
version: "1.0.0"
author: helix
tags: [search, web, research]
icon: "🔍"
---

# 网络搜索

帮助用户搜索和检索网络上的信息。

## 核心能力

- 搜索引擎查询和结果整理
- 网页内容提取和摘要
- 多源信息对比和验证
- 实时资讯获取
"#),

    ("code-assistant", r#"---
name: code-assistant
description: 代码辅助和编程技能
version: "1.0.0"
author: helix
tags: [code, programming, debug]
icon: "💻"
---

# 编程助手

帮助用户编写、调试和优化代码。

## 核心能力

- 代码编写和重构建议
- Bug 排查和修复
- 代码解释和文档生成
- 最佳实践和设计模式指导
"#),

    ("task-automation", r#"---
name: task-automation
description: 任务自动化和流程编排技能
version: "1.0.0"
author: helix
tags: [automation, workflow, task]
icon: "⚡"
---

# 任务自动化

帮助用户创建和管理自动化工作流。

## 核心能力

- 定时任务配置和管理
- 工作流编排和触发
- 脚本编写和调试
- 通知和告警设置
"#),

    ("data-analysis", r#"---
name: data-analysis
description: 数据分析和可视化技能
version: "1.0.0"
author: helix
tags: [data, analysis, visualization]
icon: "📊"
---

# 数据分析

帮助用户分析和可视化数据。

## 核心能力

- 数据清洗和转换
- 统计分析和趋势识别
- 数据可视化建议
- 报告生成和摘要
"#),
];

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Unique name (from frontmatter or directory name)
    pub name: String,
    /// Description
    pub description: String,
    /// Emoji icon
    pub icon: String,
    /// Version string
    pub version: String,
    /// Author name
    pub author: String,
    /// Tags
    pub tags: Vec<String>,
    /// Path to the SKILL.md file
    pub path: String,
    /// Whether this skill is enabled
    pub enabled: bool,
    /// The Markdown body (instructions)
    pub body: String,
    /// Homepage URL
    #[serde(default)]
    pub homepage: String,
}

// ============================================================================
// Skills directory
// ============================================================================

/// Get the skills directory: ~/.helix/skills/
fn get_skills_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    let skills_dir = home.join(".helix").join("skills");
    Ok(skills_dir)
}

/// Ensure the skills directory exists and has built-in skills.
pub fn ensure_skills_dir() -> Result<PathBuf, String> {
    let skills_dir = get_skills_dir()?;
    std::fs::create_dir_all(&skills_dir)
        .map_err(|e| format!("Failed to create skills dir: {}", e))?;

    // Auto-copy built-in skills if they don't exist
    for (name, content) in BUILTIN_SKILLS {
        let skill_dir = skills_dir.join(name);
        let skill_file = skill_dir.join("SKILL.md");
        if !skill_file.exists() {
            std::fs::create_dir_all(&skill_dir)
                .map_err(|e| format!("Failed to create skill dir '{}': {}", name, e))?;
            std::fs::write(&skill_file, content)
                .map_err(|e| format!("Failed to write skill '{}': {}", name, e))?;
            info!("Created built-in skill: {}", name);
        }
    }

    Ok(skills_dir)
}

// ============================================================================
// Database (skill states — enabled/disabled)
// ============================================================================

static SKILLS_DB: Lazy<Mutex<Connection>> = Lazy::new(|| {
    let conn = open_skills_db().expect("Failed to open skills database");
    Mutex::new(conn)
});

fn open_skills_db() -> Result<Connection, String> {
    let data_dir = get_data_dir()?;
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create data dir: {}", e))?;
    let db_path = data_dir.join("helix.db");
    let conn =
        Connection::open(&db_path).map_err(|e| format!("Failed to open skills DB: {}", e))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
        .map_err(|e| format!("Failed to set pragmas: {}", e))?;
    Ok(conn)
}

pub fn init_skills_tables() -> Result<(), String> {
    let conn = SKILLS_DB.lock();
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS skill_states (
            name    TEXT PRIMARY KEY,
            enabled INTEGER NOT NULL DEFAULT 1
        );
        ",
    )
    .map_err(|e| format!("Failed to create skill_states table: {}", e))?;
    info!("Skills tables initialized");

    // Ensure skills directory with built-ins
    drop(conn); // release lock before filesystem ops
    let _ = ensure_skills_dir();

    Ok(())
}

fn get_skill_enabled(name: &str) -> bool {
    let conn = SKILLS_DB.lock();
    conn.query_row(
        "SELECT enabled FROM skill_states WHERE name = ?1",
        params![name],
        |row| row.get::<_, i32>(0),
    )
    .map(|e| e != 0)
    .unwrap_or(true) // default enabled
}

fn set_skill_enabled(name: &str, enabled: bool) -> Result<(), String> {
    let conn = SKILLS_DB.lock();
    conn.execute(
        "INSERT INTO skill_states (name, enabled) VALUES (?1, ?2)
         ON CONFLICT(name) DO UPDATE SET enabled = ?2",
        params![name, enabled as i32],
    )
    .map_err(|e| format!("Failed to set skill state: {}", e))?;
    Ok(())
}

// ============================================================================
// SKILL.md Parser
// ============================================================================

/// Parse a SKILL.md file into frontmatter + body.
fn parse_skill_md(content: &str) -> Option<(SkillFrontmatter, String)> {
    let content = content.trim();

    if !content.starts_with("---") {
        return None;
    }

    let rest = &content[3..];
    let end_pos = rest.find("\n---")?;
    let yaml_str = &rest[..end_pos].trim();
    let body = rest[end_pos + 4..].trim().to_string();

    let frontmatter: SkillFrontmatter = serde_yaml::from_str(yaml_str).ok()?;

    Some((frontmatter, body))
}

// ============================================================================
// Filesystem Scanner
// ============================================================================

/// Scan the skills directory for SKILL.md files.
fn scan_skills() -> Vec<Skill> {
    let skills_dir = match get_skills_dir() {
        Ok(d) => d,
        Err(e) => {
            warn!("Cannot get skills dir: {}", e);
            return Vec::new();
        }
    };

    if !skills_dir.exists() {
        return Vec::new();
    }

    let mut skills = Vec::new();

    let entries = match std::fs::read_dir(&skills_dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("Failed to read skills directory: {}", e);
            return skills;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let skill_file = path.join("SKILL.md");
        if skill_file.exists() {
            if let Some(skill) = load_skill_file(&skill_file) {
                skills.push(skill);
            }
        }
    }

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// Load a single SKILL.md file.
fn load_skill_file(path: &Path) -> Option<Skill> {
    let content = std::fs::read_to_string(path).ok()?;
    let (fm, body) = parse_skill_md(&content)?;

    let name = fm.name.clone();
    let enabled = get_skill_enabled(&name);

    Some(Skill {
        name,
        description: fm.description.unwrap_or_default(),
        icon: fm.icon.unwrap_or_else(|| "📦".to_string()),
        version: fm.version.unwrap_or_else(|| "0.1.0".to_string()),
        author: fm.author.unwrap_or_else(|| "unknown".to_string()),
        tags: fm.tags.unwrap_or_default(),
        path: path.to_string_lossy().to_string(),
        enabled,
        body,
        homepage: fm.homepage.unwrap_or_default(),
    })
}

// ============================================================================
// Public API
// ============================================================================

/// List all skills from ~/.helix/skills/
pub fn list_all_skills() -> Vec<Skill> {
    scan_skills()
}

/// Get the combined system prompt for all enabled skills.
pub fn get_enabled_skills_prompt() -> String {
    let skills = list_all_skills();
    let enabled: Vec<&Skill> = skills.iter().filter(|s| s.enabled).collect();

    if enabled.is_empty() {
        return String::new();
    }

    let mut prompt = String::from("\n\n## Active Skills\n\nThe following skills are available:\n\n");

    for skill in &enabled {
        prompt.push_str(&format!(
            "### {} {}\n\n{}\n\n---\n\n",
            skill.icon, skill.name, skill.body
        ));
    }

    prompt
}

/// Toggle a skill's enabled state.
pub fn toggle_skill(name: &str, enabled: bool) -> Result<(), String> {
    set_skill_enabled(name, enabled)?;
    info!("Skill '{}' {}", name, if enabled { "enabled" } else { "disabled" });
    Ok(())
}

// ============================================================================
// Install / Uninstall / Create
// ============================================================================

/// Create a new empty skill template.
fn create_skill_template(name: &str) -> Result<String, String> {
    let skills_dir = ensure_skills_dir()?;
    let skill_dir = skills_dir.join(name);

    if skill_dir.exists() {
        return Err(format!("Skill '{}' already exists", name));
    }

    std::fs::create_dir_all(&skill_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let content = format!(
        r#"---
name: {}
description: 自定义技能描述
version: "0.1.0"
author: user
tags: [custom]
icon: "🛠️"
---

# {}

在这里编写技能指令...

## 当用户提到以下关键词时启用此技能

- 关键词1
- 关键词2

## 指导原则

- 规则1
- 规则2
"#,
        name, name
    );

    let skill_file = skill_dir.join("SKILL.md");
    std::fs::write(&skill_file, content)
        .map_err(|e| format!("Failed to write SKILL.md: {}", e))?;

    info!("Created new skill template: {}", name);
    Ok(skill_file.to_string_lossy().to_string())
}

/// Uninstall (delete) a skill.
fn uninstall_skill(name: &str) -> Result<(), String> {
    let skills_dir = get_skills_dir()?;
    let skill_dir = skills_dir.join(name);

    if !skill_dir.exists() {
        return Err(format!("Skill '{}' not found", name));
    }

    std::fs::remove_dir_all(&skill_dir)
        .map_err(|e| format!("Failed to remove skill '{}': {}", name, e))?;

    // Also remove from database
    let conn = SKILLS_DB.lock();
    let _ = conn.execute("DELETE FROM skill_states WHERE name = ?1", params![name]);

    info!("Uninstalled skill: {}", name);
    Ok(())
}

/// Install a skill from a Git URL.
fn install_from_git(url: &str) -> Result<String, String> {
    let skills_dir = ensure_skills_dir()?;

    // Extract repo name from URL for the skill directory name
    let repo_name = url
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("skill")
        .trim_end_matches(".git");

    let target_dir = skills_dir.join(repo_name);

    if target_dir.exists() {
        return Err(format!("Skill '{}' already exists. Uninstall first.", repo_name));
    }

    // Clone the repository
    let output = std::process::Command::new("git")
        .args(["clone", "--depth", "1", url, &target_dir.to_string_lossy()])
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Git clone failed: {}", stderr));
    }

    // Verify SKILL.md exists
    let skill_file = target_dir.join("SKILL.md");
    if !skill_file.exists() {
        // Cleanup
        let _ = std::fs::remove_dir_all(&target_dir);
        return Err("Repository does not contain a SKILL.md file".to_string());
    }

    info!("Installed skill from git: {} -> {}", url, repo_name);
    Ok(repo_name.to_string())
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn skills_list() -> Result<Vec<Skill>, String> {
    Ok(list_all_skills())
}

#[tauri::command]
pub async fn skills_toggle(name: String, enabled: bool) -> Result<(), String> {
    toggle_skill(&name, enabled)
}

#[tauri::command]
pub async fn skills_reload() -> Result<Vec<Skill>, String> {
    Ok(list_all_skills())
}

#[tauri::command]
pub async fn skills_get_body(name: String) -> Result<String, String> {
    let skills = list_all_skills();
    skills
        .iter()
        .find(|s| s.name == name)
        .map(|s| s.body.clone())
        .ok_or_else(|| format!("Skill '{}' not found", name))
}

#[tauri::command]
pub async fn skills_create(name: String) -> Result<String, String> {
    create_skill_template(&name)
}

#[tauri::command]
pub async fn skills_uninstall(name: String) -> Result<(), String> {
    uninstall_skill(&name)
}

#[tauri::command]
pub async fn skills_install_git(url: String) -> Result<String, String> {
    install_from_git(&url)
}

#[tauri::command]
pub async fn skills_open_dir() -> Result<String, String> {
    let skills_dir = ensure_skills_dir()?;
    let path = skills_dir.to_string_lossy().to_string();

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg(&path)
            .spawn();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn();
    }

    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg(&path)
            .spawn();
    }

    Ok(path)
}

#[tauri::command]
pub async fn skills_get_dir() -> Result<String, String> {
    let skills_dir = get_skills_dir()?;
    Ok(skills_dir.to_string_lossy().to_string())
}
