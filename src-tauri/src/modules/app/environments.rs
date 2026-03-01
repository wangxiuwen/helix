//! Environment variables manager — key-value store in ~/.helix/envs.json.
//!
//! Provides Tauri commands for managing user-defined environment variables
//! that are loaded into the agent's process environment at startup.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

/// Single environment variable entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
    /// Whether to mask the value in the UI (for secrets)
    #[serde(default)]
    pub secret: bool,
}

/// Path to the envs config file
fn get_envs_path() -> Result<std::path::PathBuf, String> {
    let helix_dir = dirs::home_dir()
        .ok_or_else(|| "Cannot determine home directory".to_string())?
        .join(".helix");
    std::fs::create_dir_all(&helix_dir)
        .map_err(|e| format!("Failed to create dir: {}", e))?;
    Ok(helix_dir.join("envs.json"))
}

/// Load env vars from file
fn load_envs() -> Result<Vec<EnvVar>, String> {
    let path = get_envs_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read envs: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse envs: {}", e))
}

/// Save env vars to file
fn save_envs(envs: &[EnvVar]) -> Result<(), String> {
    let path = get_envs_path()?;
    let content = serde_json::to_string_pretty(envs)
        .map_err(|e| format!("Failed to serialize envs: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write envs: {}", e))?;
    Ok(())
}

/// Apply env vars to the current process
pub fn apply_envs_to_process() {
    match load_envs() {
        Ok(envs) => {
            for env in &envs {
                std::env::set_var(&env.key, &env.value);
            }
            if !envs.is_empty() {
                info!("Applied {} environment variables", envs.len());
            }
        }
        Err(e) => {
            warn!("Failed to load env vars: {}", e);
        }
    }
}

/// List all environment variables
#[tauri::command]
pub async fn envs_list() -> Result<Vec<EnvVar>, String> {
    load_envs()
}

/// Set an environment variable
#[tauri::command]
pub async fn envs_set(key: String, value: String, secret: Option<bool>) -> Result<(), String> {
    let mut envs = load_envs()?;

    // Update existing or add new
    if let Some(existing) = envs.iter_mut().find(|e| e.key == key) {
        existing.value = value.clone();
        existing.secret = secret.unwrap_or(existing.secret);
    } else {
        envs.push(EnvVar {
            key: key.clone(),
            value: value.clone(),
            secret: secret.unwrap_or(false),
        });
    }

    save_envs(&envs)?;

    // Also apply to current process
    std::env::set_var(&key, &value);
    info!("Environment variable set: {}", key);
    Ok(())
}

/// Delete an environment variable
#[tauri::command]
pub async fn envs_delete(key: String) -> Result<(), String> {
    let mut envs = load_envs()?;
    envs.retain(|e| e.key != key);
    save_envs(&envs)?;

    std::env::remove_var(&key);
    info!("Environment variable deleted: {}", key);
    Ok(())
}
