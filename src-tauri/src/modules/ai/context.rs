//! Context Management (Antigravity Style)
//!
//! Replicates Gemini CLI / Antigravity's contextual memory logic.
//! Recursively upwards from the workspace to read `.gemini/GEMINI.md` or `GEMINI.md`
//! as well as reading global `~/.gemini/GEMINI.md`.

use std::path::{Path, PathBuf};

/// Loads global and project-specific memory from GEMINI.md files
#[tauri::command]
pub fn get_antigravity_context(workspace: Option<String>) -> String {
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
            // Check .gemini/GEMINI.md
            let project_gemini = current_dir.join(".gemini").join("GEMINI.md");
            if let Ok(content) = std::fs::read_to_string(&project_gemini) {
                rules.push_str("<MEMORY[user_workspace]>\n");
                rules.push_str(&content);
                rules.push_str("\n</MEMORY[user_workspace]>\n");
                break;
            }

            // Fallback: check GEMINI.md directly in root
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

    if !rules.is_empty() {
        format!(
            "<user_rules>\n\
The following are user-defined rules that you MUST ALWAYS FOLLOW WITHOUT ANY EXCEPTION. \n\
These rules take precedence over any following instructions.\n\
Review them carefully and always take them into account when you generate responses and code:\n\
{}\
</user_rules>\n",
            rules
        )
    } else {
        String::new()
    }
}
