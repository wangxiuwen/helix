//! Workspace file manager — manages ~/.helix/ prompt configuration files.
//!
//! Provides Tauri commands for listing, reading, writing, uploading, and
//! downloading files in the user's ~/.helix/ workspace directory.

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// File metadata for workspace listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFile {
    pub name: String,
    pub size: u64,
    pub modified: String,
}

/// Get the workspace directory path (~/.helix/)
fn get_workspace_dir() -> Result<std::path::PathBuf, String> {
    let helix_dir = dirs::home_dir()
        .ok_or_else(|| "Cannot determine home directory".to_string())?
        .join(".helix");
    std::fs::create_dir_all(&helix_dir)
        .map_err(|e| format!("Failed to create workspace dir: {}", e))?;
    Ok(helix_dir)
}

/// List all files in the workspace
#[tauri::command]
pub async fn workspace_list_files() -> Result<Vec<WorkspaceFile>, String> {
    let dir = get_workspace_dir()?;
    let mut files = Vec::new();

    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("Failed to read workspace dir: {}", e))?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Only show MD and common config files
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !matches!(
            ext.as_str(),
            "md" | "json" | "toml" | "yaml" | "yml" | "txt"
        ) {
            continue;
        }

        let metadata =
            std::fs::metadata(&path).map_err(|e| format!("Failed to read metadata: {}", e))?;

        let modified = metadata
            .modified()
            .map(|t| {
                let datetime: chrono::DateTime<chrono::Utc> = t.into();
                datetime.to_rfc3339()
            })
            .unwrap_or_else(|_| "unknown".to_string());

        files.push(WorkspaceFile {
            name,
            size: metadata.len(),
            modified,
        });
    }

    // Sort: core MD files first, then alphabetically
    let core_files = [
        "SOUL.md",
        "AGENTS.md",
        "PROFILE.md",
        "MEMORY.md",
        "HEARTBEAT.md",
    ];
    files.sort_by(|a, b| {
        let a_core = core_files.iter().position(|&f| f == a.name);
        let b_core = core_files.iter().position(|&f| f == b.name);
        match (a_core, b_core) {
            (Some(ai), Some(bi)) => ai.cmp(&bi),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        }
    });

    Ok(files)
}

/// Read a workspace file
#[tauri::command]
pub async fn workspace_read_file(name: String) -> Result<String, String> {
    let dir = get_workspace_dir()?;
    let path = dir.join(&name);

    // Security: ensure the file is within the workspace
    if !path.starts_with(&dir) {
        return Err("Access denied: path traversal".to_string());
    }

    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file '{}': {}", name, e))
}

/// Write a workspace file
#[tauri::command]
pub async fn workspace_write_file(name: String, content: String) -> Result<(), String> {
    let dir = get_workspace_dir()?;
    let path = dir.join(&name);

    if !path.starts_with(&dir) {
        return Err("Access denied: path traversal".to_string());
    }

    std::fs::write(&path, &content)
        .map_err(|e| format!("Failed to write file '{}': {}", name, e))?;

    info!("Workspace file written: {}", name);
    Ok(())
}

/// Delete a workspace file
#[tauri::command]
pub async fn workspace_delete_file(name: String) -> Result<(), String> {
    let dir = get_workspace_dir()?;
    let path = dir.join(&name);

    if !path.starts_with(&dir) {
        return Err("Access denied: path traversal".to_string());
    }

    // Don't allow deleting core files
    let core_files = ["AGENTS.md", "PROFILE.md"];
    if core_files.contains(&name.as_str()) {
        return Err(format!("Cannot delete core file: {}", name));
    }

    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Failed to delete '{}': {}", name, e))?;
        info!("Workspace file deleted: {}", name);
    }
    Ok(())
}

/// Get the workspace directory path (for frontend use)
#[tauri::command]
pub async fn workspace_get_dir() -> Result<String, String> {
    let dir = get_workspace_dir()?;
    Ok(dir.to_string_lossy().to_string())
}

/// Open workspace directory in native file explorer
#[tauri::command]
pub async fn workspace_open_dir(path: Option<String>) -> Result<(), String> {
    let target = if let Some(p) = path {
        let expanded = crate::modules::agent::tools::expand_path(&p);
        std::path::PathBuf::from(expanded)
    } else {
        get_workspace_dir()?
    };

    let _ = std::fs::create_dir_all(&target);

    // Open it using open crate or native command
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open explorer: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open finder: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to xdg-open: {}", e))?;
    }

    Ok(())
}

/// List files in an arbitrary directory (e.g. session workspace)
#[tauri::command]
pub async fn workspace_list_session_files(path: String) -> Result<Vec<WorkspaceFile>, String> {
    let expanded = crate::modules::agent::tools::expand_path(&path);
    let target = std::path::PathBuf::from(expanded);

    let mut files = Vec::new();
    if !target.exists() || !target.is_dir() {
        return Ok(files);
    }

    let entries = std::fs::read_dir(&target).map_err(|e| format!("Failed to read dir: {}", e))?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let p = entry.path();
        if !p.is_file() {
            continue;
        }

        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let metadata =
            std::fs::metadata(&p).map_err(|e| format!("Failed to read metadata: {}", e))?;

        let modified = metadata
            .modified()
            .map(|t| {
                let datetime: chrono::DateTime<chrono::Utc> = t.into();
                datetime.to_rfc3339()
            })
            .unwrap_or_else(|_| "unknown".to_string());

        files.push(WorkspaceFile {
            name,
            size: metadata.len(),
            modified,
        });
    }

    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

/// Read file in an arbitrary directory
#[tauri::command]
pub async fn workspace_read_session_file(dir_path: String, name: String) -> Result<String, String> {
    let expanded = crate::modules::agent::tools::expand_path(&dir_path);
    let target = std::path::PathBuf::from(expanded).join(&name);

    std::fs::read_to_string(&target).map_err(|e| format!("Failed to read file '{}': {}", name, e))
}
