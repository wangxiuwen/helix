use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tracing::{info, warn};
use uuid::Uuid;
use chrono::Utc;
use once_cell::sync::Lazy;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanDevice {
    #[serde(default)]
    pub ip: String,
    pub alias: String,
    pub version: Option<String>,
    pub device_model: Option<String>,
    pub device_type: Option<String>,
    pub fingerprint: String,
    pub port: Option<u16>,
    pub protocol: Option<String>,
    pub download: Option<bool>,
    pub announce: Option<bool>,
    pub announcement: Option<bool>,
    #[serde(skip)]
    pub last_seen: i64,
}

pub type PeerMap = Arc<Mutex<HashMap<String, LanDevice>>>;

pub static LAN_PEERS: Lazy<PeerMap> = Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

pub async fn start_udp_discovery(alias: String, port: u16) -> anyhow::Result<()> {
    let multicast_addr = "224.0.0.167".parse::<Ipv4Addr>()?;
    let bind_addr: SocketAddr = "0.0.0.0:53317".parse()?;

    // Create a standard library UDP socket first to set low-level options
    let std_socket = std::net::UdpSocket::bind(bind_addr)?;
    std_socket.set_nonblocking(true)?;
    
    // Broadcast is needed for 255.255.255.255 fallback
    if let Err(e) = std_socket.set_broadcast(true) {
        warn!("Could not enable UDP broadcast: {}", e);
    }
    
    // Join multicast group for 224.0.0.167
    if let Err(e) = std_socket.join_multicast_v4(&multicast_addr, &Ipv4Addr::UNSPECIFIED) {
        warn!("Failed to join UDP multicast (may default to broadcast): {}", e);
    }

    let fingerprint = Uuid::new_v4().to_string();
    
    // 1. Announcer thread
    let alias_clone = alias.clone();
    let fp_clone = fingerprint.clone();
    let std_socket_clone1 = std_socket.try_clone()?;
    tokio::spawn(async move {
        // Convert to Tokio socket INSIDE the async runtime to avoid "no reactor running" panic
        let socket_send = match UdpSocket::from_std(std_socket_clone1) {
            Ok(s) => Arc::new(s),
            Err(e) => {
                warn!("Failed to convert UDP socket to tokio: {}", e);
                return;
            }
        };

        info!("Starting UDP LAN Announcer...");
        let payload = json!({
            "alias": alias_clone,
            "version": "2.0",
            "deviceModel": "Helix Agent",
            "deviceType": "desktop",
            "fingerprint": fp_clone,
            "port": port,
            "protocol": "http",
            "download": true,
            "announce": true,
            "announcement": true
        });
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        
        let target_multicast: SocketAddr = "224.0.0.167:53317".parse().unwrap();
        let target_broadcast: SocketAddr = "255.255.255.255:53317".parse().unwrap();

        loop {
            let _ = socket_send.send_to(&payload_bytes, target_multicast).await;
            let _ = socket_send.send_to(&payload_bytes, target_broadcast).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        }
    });

    // 2. Listener loop
    let cleanup_peers = LAN_PEERS.clone();
    // Try cloning the original std socket again for the listener
    let std_socket_clone2 = std_socket.try_clone()?;
    let fp_clone2 = fingerprint.clone();
    tokio::spawn(async move {
        let socket_recv = match UdpSocket::from_std(std_socket_clone2) {
            Ok(s) => Arc::new(s),
            Err(e) => return,
        };

        info!("Starting UDP LAN Listener...");
        let mut buf = vec![0u8; 4096];
        loop {
            match socket_recv.recv_from(&mut buf).await {
                Ok((len, peer_addr)) => {
                    let ip = peer_addr.ip().to_string();
                    if let Ok(mut device) = serde_json::from_slice::<LanDevice>(&buf[..len]) {
                        if device.fingerprint != fp_clone2 {
                            device.ip = ip;
                            device.last_seen = Utc::now().timestamp();
                            let mut map = cleanup_peers.lock().await;
                            map.insert(device.fingerprint.clone(), device);
                        }
                    }
                }
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
    });
    
    // 3. Cleanup dead peers > 30s
    let cleanup_peers2 = LAN_PEERS.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            let now = Utc::now().timestamp();
            let mut map = cleanup_peers2.lock().await;
            map.retain(|_, v| now - v.last_seen < 30);
        }
    });

    Ok(())
}
