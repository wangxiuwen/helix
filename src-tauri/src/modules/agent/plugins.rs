//! Plugin Ecosystem — Hot-swappable external tools via JSON-RPC over stdio
//!
//! Scans a plugin directory, parses tool manifests from executables,
//! and provides execution routing for external tools.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tracing::{error, info, warn};

use crate::modules::config::get_data_dir;

/// Plugin tool definition — local copy for manifest parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub r#type: String,
    pub function: ToolFunctionDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone)]
pub struct PluginRegistry {
    /// Maps tool_name -> executable path
    pub tools: HashMap<String, PathBuf>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Scan plugins directory and load all manifests
    pub async fn load_plugins() -> Self {
        let mut registry = Self::new();
        let plugin_dir: PathBuf = match get_data_dir() {
            Ok(mut p) => {
                p.push("plugins");
                p
            }
            Err(_) => return registry,
        };

        if !plugin_dir.exists() {
            let _ = tokio::fs::create_dir_all(&plugin_dir).await;
            return registry;
        }

        let mut entries: tokio::fs::ReadDir = match tokio::fs::read_dir(&plugin_dir).await {
            Ok(e) => e,
            Err(e) => {
                error!("Failed to read plugins dir: {}", e);
                return registry;
            }
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            // We only load executable files (or scripts with shebangs)
            if path.is_file() {
                if is_executable(&path) {
                    registry.discover_tools(&path).await;
                }
            }
        }

        info!("Loaded {} plugin tools", registry.tools.len());
        registry
    }

    /// Ask an executable for its manifest via `--manifest` flag
    async fn discover_tools(&mut self, path: &PathBuf) {
        let cmd = tokio::process::Command::new(path)
            .arg("--manifest")
            .output()
            .await;

        match cmd {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Ok(manifest) = serde_json::from_str::<PluginManifest>(&stdout) {
                    for tool in manifest.tools {
                        let tool_name = tool.function.name.clone();
                        info!("Registered plugin tool: {} from {:?}", tool_name, path);
                        self.tools.insert(tool_name, path.clone());
                    }
                } else {
                    warn!("Invalid JSON manifest from plugin: {:?}", path);
                }
            }
            Ok(output) => {
                warn!("Plugin {:?} returned non-zero for --manifest: {}", path, output.status);
            }
            Err(e) => {
                warn!("Failed to execute plugin {:?} for manifest: {}", path, e);
            }
        }
    }

    /// Get all tool definitions combining native tools + plugin tools
    pub async fn get_all_tool_definitions(native: Vec<ToolDefinition>) -> Vec<ToolDefinition> {
        let mut all = native;
        
        // Load dynamically (in a real app, this might be cached and reloaded via UI)
        let registry = Self::load_plugins().await;
        
        for (name, path) in registry.tools {
            // We don't have the full schema stored in the map to save memory, 
            // but we could. For simplicity, we just fetch it again or cache it.
            // Let's refetch it for now, though it's slow.
            // Optimally, PluginRegistry would cache the ToolDefinition.
            if let Ok(output) = tokio::process::Command::new(&path).arg("--manifest").output().await {
                if let Ok(manifest) = serde_json::from_str::<PluginManifest>(&String::from_utf8_lossy(&output.stdout)) {
                     for tool in manifest.tools {
                         if tool.function.name == name {
                             all.push(tool);
                         }
                     }
                }
            }
        }
        
        all
    }

    /// Execute a specific plugin tool via JSON-RPC over stdio
    pub async fn execute_tool(path: &PathBuf, tool_name: &str, args: &Value) -> Result<String, String> {
        // RPC Request Format
        let request = json!({
            "jsonrpc": "2.0",
            "method": tool_name,
            "params": args,
            "id": 1
        });

        let mut child = tokio::process::Command::new(path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Spawn plugin failed: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            let req_str = serde_json::to_string(&request).unwrap() + "\n";
            stdin.write_all(req_str.as_bytes()).await.map_err(|e| e.to_string())?;
        }

        // Wait for stdout JSON-RPC response (timeout 30s)
        let output = tokio::time::timeout(std::time::Duration::from_secs(30), child.wait_with_output()).await;
        
        match output {
            Ok(Ok(out)) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                
                // Parse JSON-RPC Response
                if let Ok(resp) = serde_json::from_str::<Value>(&stdout) {
                    if let Some(error) = resp.get("error") {
                        return Err(error.to_string());
                    }
                    if let Some(result) = resp.get("result") {
                        if let Some(s) = result.as_str() {
                            return Ok(s.to_string());
                        }
                        return Ok(result.to_string());
                    }
                }
                
                // Fallback to raw string if plugin didn't strictly follow JSON-RPC
                Ok(stdout.trim().to_string())
            }
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err("Plugin execution timed out".into())
        }
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(path) {
        metadata.permissions().mode() & 0o111 != 0
    } else {
        false
    }
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        ext == "exe" || ext == "bat" || ext == "cmd" || ext == "ps1"
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_execution() {
        let home = std::env::var("HOME").unwrap();
        let plugin_path = std::path::PathBuf::from(home).join(".helix/plugins/test_plugin.py");

        if !plugin_path.exists() {
            println!("test_plugin.py not found, skipping plugin test");
            return;
        }

        // Test manifest discovery
        let mut registry = PluginRegistry::new();
        registry.discover_tools(&plugin_path).await;
        
        assert!(registry.tools.contains_key("plugin_hello_world"));

        // Test plugin execution via JSON-RPC
        use serde_json::json;
        let args = json!({
            "name": "Cargo Test"
        });

        let result = PluginRegistry::execute_tool(&plugin_path, "plugin_hello_world", &args).await;
        assert!(result.is_ok());
        let res_str = result.unwrap();
        assert_eq!(res_str, "Hello Cargo Test from Python Plugin!");
        println!("Plugin execution passed: {}", res_str);
    }
}
