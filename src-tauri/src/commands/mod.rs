use crate::models::AppConfig;
use crate::modules;

// 导出 autostart 命令
pub mod autostart;
// 导出 cloudflared 命令
pub mod cloudflared;

/// 加载配置
#[tauri::command]
pub async fn load_config() -> Result<AppConfig, String> {
    modules::load_app_config()
}

/// 保存配置
#[tauri::command]
pub async fn save_config(
    app: tauri::AppHandle,
    config: AppConfig,
) -> Result<(), String> {
    modules::save_app_config(&config)?;

    // 通知托盘配置已更新
    let _ = tauri::Emitter::emit(&app, "config://updated", ());

    Ok(())
}

/// 清理日志缓存
#[tauri::command]
pub async fn clear_log_cache() -> Result<(), String> {
    modules::logger::clear_logs()
}

/// 显示主窗口
#[tauri::command]
pub async fn show_main_window(window: tauri::Window) -> Result<(), String> {
    window.show().map_err(|e| e.to_string())
}

/// 设置窗口主题（用于同步 Windows 标题栏按钮颜色）
#[tauri::command]
pub async fn set_window_theme(window: tauri::Window, theme: String) -> Result<(), String> {
    use tauri::Theme;

    let tauri_theme = match theme.as_str() {
        "dark" => Some(Theme::Dark),
        "light" => Some(Theme::Light),
        _ => None, // system default
    };

    window.set_theme(tauri_theme).map_err(|e| e.to_string())
}

/// 检测更新响应结构
pub use crate::modules::update_checker::UpdateInfo;

/// 检测 GitHub releases 更新
#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    modules::logger::log_info("收到前端触发的更新检查请求");
    crate::modules::update_checker::check_for_updates().await
}

#[tauri::command]
pub async fn should_check_updates() -> Result<bool, String> {
    let settings = crate::modules::update_checker::load_update_settings()?;
    Ok(crate::modules::update_checker::should_check_for_updates(
        &settings,
    ))
}

#[tauri::command]
pub async fn update_last_check_time() -> Result<(), String> {
    crate::modules::update_checker::update_last_check_time()
}

/// 检测是否通过 Homebrew Cask 安装
#[tauri::command]
pub async fn check_homebrew_installation() -> Result<bool, String> {
    Ok(crate::modules::update_checker::is_homebrew_installed())
}

/// 通过 Homebrew Cask 升级应用
#[tauri::command]
pub async fn brew_upgrade_cask() -> Result<String, String> {
    modules::logger::log_info("收到前端触发的 Homebrew 升级请求");
    crate::modules::update_checker::brew_upgrade_cask().await
}

/// 获取更新设置
#[tauri::command]
pub async fn get_update_settings() -> Result<crate::modules::update_checker::UpdateSettings, String>
{
    crate::modules::update_checker::load_update_settings()
}

/// 保存更新设置
#[tauri::command]
pub async fn save_update_settings(
    settings: crate::modules::update_checker::UpdateSettings,
) -> Result<(), String> {
    crate::modules::update_checker::save_update_settings(&settings)
}

fn validate_path(path: &str) -> Result<(), String> {
    if path.contains("..") {
        return Err("非法路径: 不允许目录遍历".to_string());
    }

    let lower_path = path.to_lowercase();
    let sensitive_prefixes = [
        "/etc/",
        "/var/spool/cron",
        "/root/",
        "/proc/",
        "/sys/",
        "/dev/",
        "c:\\windows",
        "c:\\users\\administrator",
        "c:\\pagefile.sys",
    ];

    for prefix in sensitive_prefixes {
        if lower_path.starts_with(prefix) {
            return Err(format!("安全拒绝: 禁止访问系统敏感路径 ({})", prefix));
        }
    }

    Ok(())
}

/// 保存文本文件
#[tauri::command]
pub async fn save_text_file(path: String, content: String) -> Result<(), String> {
    validate_path(&path)?;
    std::fs::write(&path, content).map_err(|e| format!("写入文件失败: {}", e))
}

/// 读取文本文件
#[tauri::command]
pub async fn read_text_file(path: String) -> Result<String, String> {
    validate_path(&path)?;
    std::fs::read_to_string(&path).map_err(|e| format!("读取文件失败: {}", e))
}

// ============================================================================
// K8s / Aliyun 配置读取命令
// ============================================================================

/// 读取 kubeconfig 信息
#[tauri::command]
pub async fn get_kube_info(custom_path: Option<String>) -> Result<modules::kubeconfig::KubeInfo, String> {
    modules::kubeconfig::load_kube_info(custom_path.as_deref())
}

/// 读取阿里云配置信息
#[tauri::command]
pub async fn get_aliyun_info() -> Result<modules::aliyun::AliyunInfo, String> {
    modules::aliyun::load_aliyun_info()
}
