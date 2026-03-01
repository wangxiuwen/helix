//! MCP (Model Context Protocol) client manager.
//!
//! Manages MCP client configurations stored in ~/.helix/mcp.json.
//! Supports stdio and SSE transport types.

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// MCP client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPClient {
    pub name: String,
    /// Transport type: "stdio" or "sse"
    pub transport: String,
    /// Command to run (for stdio transport)
    pub command: Option<String>,
    /// Command arguments (for stdio transport)
    pub args: Option<Vec<String>>,
    /// URL endpoint (for sse transport)
    pub url: Option<String>,
    /// Environment variables for the MCP process
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// Whether this client is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

/// MCP configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MCPConfig {
    #[serde(default)]
    clients: Vec<MCPClient>,
}

/// Path to the MCP config file
fn get_mcp_config_path() -> Result<std::path::PathBuf, String> {
    let helix_dir = dirs::home_dir()
        .ok_or_else(|| "Cannot determine home directory".to_string())?
        .join(".helix");
    std::fs::create_dir_all(&helix_dir)
        .map_err(|e| format!("Failed to create dir: {}", e))?;
    Ok(helix_dir.join("mcp.json"))
}

/// Load MCP config
fn load_mcp_config() -> Result<MCPConfig, String> {
    let path = get_mcp_config_path()?;
    if !path.exists() {
        return Ok(MCPConfig::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read MCP config: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse MCP config: {}", e))
}

/// Save MCP config
fn save_mcp_config(config: &MCPConfig) -> Result<(), String> {
    let path = get_mcp_config_path()?;
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize MCP config: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write MCP config: {}", e))?;
    Ok(())
}

/// List all MCP clients
#[tauri::command]
pub async fn mcp_list() -> Result<Vec<MCPClient>, String> {
    let config = load_mcp_config()?;
    Ok(config.clients)
}

/// Create a new MCP client
#[tauri::command]
pub async fn mcp_create(client: MCPClient) -> Result<MCPClient, String> {
    let mut config = load_mcp_config()?;

    // Check for duplicate name
    if config.clients.iter().any(|c| c.name == client.name) {
        return Err(format!("MCP client '{}' already exists", client.name));
    }

    // Validate transport
    match client.transport.as_str() {
        "stdio" => {
            if client.command.is_none() || client.command.as_ref().map(|c| c.is_empty()).unwrap_or(true) {
                return Err("stdio transport requires a command".to_string());
            }
        }
        "sse" => {
            if client.url.is_none() || client.url.as_ref().map(|u| u.is_empty()).unwrap_or(true) {
                return Err("sse transport requires a URL".to_string());
            }
        }
        _ => return Err(format!("Unknown transport type: {}", client.transport)),
    }

    info!("Created MCP client: {} ({})", client.name, client.transport);
    config.clients.push(client.clone());
    save_mcp_config(&config)?;

    Ok(client)
}

/// Toggle MCP client enabled/disabled
#[tauri::command]
pub async fn mcp_toggle(name: String) -> Result<MCPClient, String> {
    let mut config = load_mcp_config()?;

    let client = config.clients.iter_mut()
        .find(|c| c.name == name)
        .ok_or_else(|| format!("MCP client '{}' not found", name))?;

    client.enabled = !client.enabled;
    let result = client.clone();
    info!("MCP client '{}' {}", name, if result.enabled { "enabled" } else { "disabled" });

    save_mcp_config(&config)?;
    Ok(result)
}

/// Delete an MCP client
#[tauri::command]
pub async fn mcp_delete(name: String) -> Result<(), String> {
    let mut config = load_mcp_config()?;
    let before = config.clients.len();
    config.clients.retain(|c| c.name != name);

    if config.clients.len() == before {
        return Err(format!("MCP client '{}' not found", name));
    }

    save_mcp_config(&config)?;
    info!("Deleted MCP client: {}", name);
    Ok(())
}

/// Update an MCP client
#[tauri::command]
pub async fn mcp_update(name: String, client: MCPClient) -> Result<MCPClient, String> {
    let mut config = load_mcp_config()?;

    let existing = config.clients.iter_mut()
        .find(|c| c.name == name)
        .ok_or_else(|| format!("MCP client '{}' not found", name))?;

    *existing = client.clone();
    save_mcp_config(&config)?;
    info!("Updated MCP client: {}", name);
    Ok(client)
}

/// Get list of tools from enabled MCP clients (for agent tool injection)
pub fn get_enabled_mcp_tool_descriptions() -> String {
    let config = match load_mcp_config() {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    let enabled: Vec<&MCPClient> = config.clients.iter()
        .filter(|c| c.enabled)
        .collect();

    if enabled.is_empty() {
        return String::new();
    }

    let mut desc = String::from("## MCP Tools\n\nThe following MCP (Model Context Protocol) clients are connected:\n\n");
    for client in &enabled {
        desc.push_str(&format!("- **{}** ({})", client.name, client.transport));
        if let Some(ref url) = client.url {
            desc.push_str(&format!(" — {}", url));
        }
        if let Some(ref cmd) = client.command {
            desc.push_str(&format!(" — `{}`", cmd));
        }
        desc.push('\n');
    }

    desc
}
