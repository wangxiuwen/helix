use super::udp_discovery::{LanDevice, LAN_PEERS};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[tauri::command]
pub async fn get_lan_peers() -> Result<Vec<LanDevice>, String> {
    let map = LAN_PEERS.lock().await;
    Ok(map.values().cloned().collect())
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OutgoingMessage {
    pub session_id: String,
    pub role: String,
    pub name: String,
    pub content: String,
    pub reply_to: Option<String>,
}

#[tauri::command]
pub async fn send_lan_message(ip: String, port: u16, payload: OutgoingMessage) -> Result<(), String> {
    let url = format!("http://{}:{}/api/helix/v1/message", ip, port);
    // Timeout set to 3 seconds for fast failure on dead peers
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;
        
    let res = client.post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Network Error: {}", e))?;
        
    if !res.status().is_success() {
        return Err(format!("LAN message failed with status: {}", res.status()));
    }
    
    Ok(())
}
