use crate::modules::{config, logger};

/// Start the background scheduler for periodic tasks
pub fn start_scheduler(app_handle: Option<tauri::AppHandle>) {
    let _app = app_handle;
    
    tauri::async_runtime::spawn(async move {
        logger::log_info("Scheduler started");
        
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // 5 minutes
        
        loop {
            interval.tick().await;
            
            // Periodic config reload check
            if let Ok(_config) = config::load_app_config() {
                // Future: add periodic ops tasks here (kubeconfig refresh, aliyun config check, etc.)
            }
        }
    });
}

