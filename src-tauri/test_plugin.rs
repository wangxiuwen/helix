#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    println!("Testing PluginRegistry...");

    // Normally this requires AppHandle, but we can mock or just test the pure function part?
    // Wait, PluginRegistry uses get_helix_dir which might need AppConfig, but let's just use the hardcoded path for the test.
    let path = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".helix/plugins/test_plugin.py");
    
    // Test discovering tools
    let output = tokio::process::Command::new(&path).arg("--manifest").output().await.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("Manifest Output:\n{}", stdout);

    // Test executing tool
    use serde_json::json;
    let request = json!({
        "jsonrpc": "2.0",
        "method": "plugin_hello_world",
        "params": {
            "name": "Helix Developer"
        },
        "id": 1
    });

    use tokio::io::AsyncWriteExt;
    let mut child = tokio::process::Command::new(&path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn().unwrap();

    if let Some(mut stdin) = child.stdin.take() {
        let req_str = serde_json::to_string(&request).unwrap() + "\n";
        stdin.write_all(req_str.as_bytes()).await.unwrap();
    }

    let out = child.wait_with_output().await.unwrap();
    println!("Execution Output:\n{}", String::from_utf8_lossy(&out.stdout));
}
