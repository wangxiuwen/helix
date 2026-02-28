//! Skills Backend — SKILL.md file loader and system prompt injector.
//!
//! Skills are stored in `~/.helix/skills/<name>/SKILL.md`.
//! Each SKILL.md has YAML frontmatter (name, description, version, author, tags, icon)
//! and a Markdown body that is injected into the agent system prompt.
//! Enabled/disabled is controlled by an `enabled` field in frontmatter (default: true).
//! No database storage — skills are discovered by scanning the directory each time.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

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
    /// Whether this skill is enabled (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub icon: String,
    pub version: String,
    pub author: String,
    pub tags: Vec<String>,
    pub path: String,
    pub enabled: bool,
    pub body: String,
    #[serde(default)]
    pub homepage: String,
}

// ============================================================================
// Skills directory
// ============================================================================

/// Get the skills directory: ~/.helix/skills/
fn get_skills_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    Ok(home.join(".helix").join("skills"))
}

/// Ensure the skills directory exists.
fn ensure_skills_dir() -> Result<PathBuf, String> {
    let skills_dir = get_skills_dir()?;
    std::fs::create_dir_all(&skills_dir)
        .map_err(|e| format!("Failed to create skills dir: {}", e))?;
    Ok(skills_dir)
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
        if !path.is_dir() { continue; }
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

    Some(Skill {
        name: fm.name.clone(),
        description: fm.description.unwrap_or_default(),
        icon: fm.icon.unwrap_or_else(|| "📦".to_string()),
        version: fm.version.unwrap_or_else(|| "0.1.0".to_string()),
        author: fm.author.unwrap_or_else(|| "unknown".to_string()),
        tags: fm.tags.unwrap_or_default(),
        path: path.to_string_lossy().to_string(),
        enabled: fm.enabled,
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

/// Toggle a skill's enabled state by rewriting its SKILL.md frontmatter.
pub fn toggle_skill(name: &str, enabled: bool) -> Result<(), String> {
    let skills_dir = get_skills_dir()?;
    let skill_file = skills_dir.join(name).join("SKILL.md");

    if !skill_file.exists() {
        return Err(format!("Skill '{}' not found", name));
    }

    let content = std::fs::read_to_string(&skill_file)
        .map_err(|e| format!("Failed to read SKILL.md: {}", e))?;

    let (mut fm, body) = parse_skill_md(&content)
        .ok_or_else(|| "Failed to parse SKILL.md".to_string())?;

    fm.enabled = enabled;

    // Rewrite the file with updated frontmatter
    let yaml = serde_yaml::to_string(&fm)
        .map_err(|e| format!("Failed to serialize frontmatter: {}", e))?;
    let new_content = format!("---\n{}---\n\n{}\n", yaml, body);
    std::fs::write(&skill_file, new_content)
        .map_err(|e| format!("Failed to write SKILL.md: {}", e))?;

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
enabled: true
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

    info!("Uninstalled skill: {}", name);
    Ok(())
}

/// Install a skill from a Git URL.
fn install_from_git(url: &str) -> Result<String, String> {
    let skills_dir = ensure_skills_dir()?;

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

    let output = std::process::Command::new("git")
        .args(["clone", "--depth", "1", url, &target_dir.to_string_lossy()])
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Git clone failed: {}", stderr));
    }

    let skill_file = target_dir.join("SKILL.md");
    if !skill_file.exists() {
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
    { let _ = std::process::Command::new("open").arg(&path).spawn(); }

    #[cfg(target_os = "linux")]
    { let _ = std::process::Command::new("xdg-open").arg(&path).spawn(); }

    #[cfg(target_os = "windows")]
    { let _ = std::process::Command::new("explorer").arg(&path).spawn(); }

    Ok(path)
}

#[tauri::command]
pub async fn skills_get_dir() -> Result<String, String> {
    let skills_dir = get_skills_dir()?;
    Ok(skills_dir.to_string_lossy().to_string())
}

// ============================================================================
// Hot-Reload Watcher
// ============================================================================

/// Start a background task that scans the skills directory every 5 seconds
/// and emits a `skills-changed` event when the skill list changes.
pub fn start_skills_watcher() {
    tauri::async_runtime::spawn(async {
        use std::collections::HashSet;
        let mut last_snapshot: HashSet<String> = HashSet::new();

        // Ensure directory exists
        let _ = ensure_skills_dir();

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            let skills = scan_skills();
            let current: HashSet<String> = skills
                .iter()
                .map(|s| format!("{}:{}:{}", s.name, s.version, s.enabled))
                .collect();

            if current != last_snapshot {
                if !last_snapshot.is_empty() {
                    // Only emit after the first scan (skip initial load)
                    info!("[skills] Change detected, notifying frontend ({} skills)", skills.len());
                    let payload = serde_json::json!({ "count": skills.len() });
                    crate::modules::infra::log_bridge::emit_custom_event("skills-changed", payload);
                }
                last_snapshot = current;
            }
        }
    });
    info!("[skills] Hot-reload watcher started (scan every 5s)");
}

