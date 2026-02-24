use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::Page;
use chromiumoxide::cdp::browser_protocol::accessibility::GetFullAxTreeParams;
use futures::StreamExt;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

lazy_static::lazy_static! {
    static ref GLOBAL_BROWSER: Arc<Mutex<Option<BrowserSession>>> = Arc::new(Mutex::new(None));
}

/// Stores the current active page and the mapping of short IDs to backendNodeIds
pub struct BrowserSession {
    browser: Browser,
    active_page: Page,
    /// Maps our short `ref_id` (e.g. "e12") to actual Chrome `BackendNodeId`
    node_map: HashMap<String, chromiumoxide::cdp::browser_protocol::dom::BackendNodeId>,
}

impl BrowserSession {
    pub async fn launch() -> Result<(), String> {
        let mut global: tokio::sync::MutexGuard<'_, Option<BrowserSession>> = GLOBAL_BROWSER.lock().await;
        if global.is_some() {
            return Ok(());
        }

        info!("Launching headless Chromium via chromiumoxide...");
        
        // Find existing Chrome or use default path, and point to actual User Data Dir for cookies.
        let home = std::env::var("HOME").unwrap_or_default();
        let user_data_dir = format!("{}/Library/Application Support/Google/Chrome", home);
        
        let config = BrowserConfig::builder()
            .chrome_executable("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
            .with_head() // Show window for now so user can see it "挂机"
            .user_data_dir(user_data_dir)
            .build()
            .map_err(|e| format!("BrowserConfig Error: {}", e))?;

        let (browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| format!("Browser Launch Error: {}", e))?;
        
        // Spawn the CDP event loop
        tokio::task::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }
        });

        // Open an initial blank page
        let page = browser.new_page("about:blank")
            .await
            .map_err(|e| format!("New Page Error: {}", e))?;

        *global = Some(BrowserSession {
            browser,
            active_page: page,
            node_map: HashMap::new(),
        });

        Ok(())
    }

    pub async fn goto(url: &str) -> Result<String, String> {
        let mut global: tokio::sync::MutexGuard<'_, Option<BrowserSession>> = GLOBAL_BROWSER.lock().await;
        let session = global.as_mut().ok_or_else(|| "Browser not launched. Call browser_launch first.".to_string())?;

        info!("Browser navigating to: {}", url);
        let _ = session.active_page.goto(url)
            .await
            .map_err(|e| format!("Goto Error: {}", e))?;

        let _ = session.active_page.wait_for_navigation_response()
            .await
            .map_err(|e| format!("Navigation Wait Error: {}", e))?;
            
        // Small delay to let JS settle
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

        Self::extract_semantic_tree(session).await
    }

    async fn extract_semantic_tree(session: &mut BrowserSession) -> Result<String, String> {
        // Here we call CDP Accessibility.getFullAXTree
        let params = GetFullAxTreeParams::default();
        let response = session.active_page
            .execute(params)
            .await
            .map_err(|e| format!("AXTree Error: {}", e))?;
        let result = response.result;

        // Format into Markdown and populate node_map
        session.node_map.clear();
        let mut md = String::new();
        let mut counter = 1;

        for node in result.nodes {
            // Determine if it's an interactive or meaningful role
            let role = node.role.as_ref().and_then(|r| r.value.as_ref()).and_then(|v| v.as_str()).unwrap_or("");
            
            // Filter roles that openclaw / claude computer use cares about
            let is_interactive = matches!(
                role,
                "button" | "link" | "textbox" | "searchbox" | "checkbox" | "combobox" | "heading" | "StaticText"
            );

            if !is_interactive {
                continue;
            }

            // Extract name
            let name = node.name.as_ref().and_then(|n| n.value.as_ref()).and_then(|v| v.as_str()).unwrap_or("").trim();
            if name.is_empty() && role != "textbox" {
                continue; // Skip useless invisible nodes unless it's a textbox!
            }

            let backend_id = match node.backend_dom_node_id {
                Some(id) => id,
                None => continue,
            };

            let ref_id = format!("e{}", counter);
            counter += 1;

            session.node_map.insert(ref_id.clone(), backend_id);

            // e.g. - button "Login" [ref=e1]
            md.push_str(&format!("- {} \"{}\" [ref={}]\n", role, name, ref_id));
        }

        if md.is_empty() {
            md.push_str("(No interactive elements found on page)");
        }

        Ok(md)
    }

    pub async fn click(ref_id: &str) -> Result<String, String> {
        let mut global: tokio::sync::MutexGuard<'_, Option<BrowserSession>> = GLOBAL_BROWSER.lock().await;
        let session = global.as_mut().ok_or_else(|| "Browser not launched.".to_string())?;

        let backend_node_id = session.node_map.get(ref_id)
            .ok_or_else(|| format!("Invalid ref_id: {}", ref_id))?;

        // 1. Resolve to RemoteObjectId
        let resolve_params = chromiumoxide::cdp::browser_protocol::dom::ResolveNodeParams::builder()
            .backend_node_id(backend_node_id.clone())
            .build();
            
        let response = session.active_page.execute(resolve_params).await.map_err(|e| format!("Resolve Node Error: {}", e))?;
        let remote_obj = response.result.object;
        let object_id = remote_obj.object_id.ok_or("No object_id")?;

        // 2. Call JS on the node
        let js = "function() { this.scrollIntoViewIfNeeded(); this.click(); return 'Clicked'; }";
        let call_params = chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams::builder()
            .object_id(object_id)
            .function_declaration(js.to_string())
            .build()
            .map_err(|e| format!("Param Builder Error: {}", e))?;
            
        session.active_page.execute(call_params).await.map_err(|e| format!("JS Exec Error: {}", e))?;

        Ok(format!("Clicked element {}", ref_id))
    }

    pub async fn fill(ref_id: &str, text: &str) -> Result<String, String> {
        let mut global: tokio::sync::MutexGuard<'_, Option<BrowserSession>> = GLOBAL_BROWSER.lock().await;
        let session = global.as_mut().ok_or_else(|| "Browser not launched.".to_string())?;

        let backend_node_id = session.node_map.get(ref_id)
            .ok_or_else(|| format!("Invalid ref_id: {}", ref_id))?;

        // 1. Resolve to RemoteObjectId
        let resolve_params = chromiumoxide::cdp::browser_protocol::dom::ResolveNodeParams::builder()
            .backend_node_id(backend_node_id.clone())
            .build();
            
        let response = session.active_page.execute(resolve_params).await.map_err(|e| format!("Resolve Node Error: {}", e))?;
        let remote_obj = response.result.object;
        let object_id = remote_obj.object_id.ok_or("No object_id")?;

        // 2. Call JS on the node
        let safe_text = text.replace("'", "\\'").replace("\n", "\\n");
        let js = format!("function() {{ this.focus(); this.value = '{}'; this.dispatchEvent(new Event('input', {{ bubbles: true }})); this.dispatchEvent(new Event('change', {{ bubbles: true }})); return 'Filled'; }}", safe_text);
        let call_params = chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams::builder()
            .object_id(object_id)
            .function_declaration(js)
            .build()
            .map_err(|e| format!("Param Builder Error: {}", e))?;
            
        session.active_page.execute(call_params).await.map_err(|e| format!("JS Exec Error: {}", e))?;

        Ok(format!("Filled text into {}", ref_id))
    }
}
