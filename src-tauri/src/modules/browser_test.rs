#[cfg(test)]
mod tests {
    use crate::modules::browser_engine::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_browser_automation() {
        // We only want to print the test log directly
        let _ = tracing_subscriber::fmt::try_init();

        println!("=== Starting Browser Automation Test ===");
        println!("Note: If you have Google Chrome open, this might fail because of User Data Dir locks!");

        match BrowserSession::launch().await {
            Ok(_) => {
                println!("Browser launched successfully with Default User Data.");
            }
            Err(e) => {
                println!("Failed to launch browser: {}", e);
                println!("TIP: Please completely QUIT Google Chrome (Cmd+Q) and try again.");
                return;
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        println!("Navigating to Hacker News...");
        match BrowserSession::goto("https://news.ycombinator.com/").await {
            Ok(ax_tree) => {
                println!("Extracted Semantic AXTree:");
                println!("{}", ax_tree);

                // Let's see if we can find a link to click!
                // format: - link "submit" [ref=eX]
                let submit_link: Option<String> = ax_tree.lines().find(|l: &&str| l.contains("submit") && l.contains("link")).map(|l: &str| {
                    let start = l.find("[ref=").unwrap() + 5;
                    let end = l.find("]").unwrap();
                    l[start..end].to_string()
                });

                if let Some(ref_id) = submit_link {
                    println!("Found 'submit' link with ref_id: {}. Clicking it...", ref_id);
                    match BrowserSession::click(&ref_id).await {
                        Ok(res) => println!("Click result: {}", res),
                        Err(e) => println!("Click failed: {}", e),
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                } else {
                    println!("Could not find the 'submit' link in the AXTree.");
                }
            }
            Err(e) => {
                println!("Goto failed: {}", e);
            }
        }
        
        println!("=== Test Complete ===");
    }
}
