//! Security Audit — Configuration, filesystem, and tool policy auditing.
//!
//! Ported from OpenClaw `src/security/audit.ts`: checks for exposed API keys,
//! insecure permissions, dangerous tool configs, and generates a structured report.

use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tracing::info;

use super::config::get_data_dir;
use super::load_app_config;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    #[serde(rename = "critical")]
    Critical,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "info")]
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFinding {
    pub check_id: String,
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    #[serde(default)]
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummary {
    pub critical: usize,
    pub warning: usize,
    pub info: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub timestamp: String,
    pub summary: AuditSummary,
    pub findings: Vec<AuditFinding>,
    pub system_info: SystemInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub user: String,
    pub home: String,
    pub data_dir: String,
}

// ============================================================================
// Audit Checks
// ============================================================================

fn check_api_key_security() -> Vec<AuditFinding> {
    let mut findings = Vec::new();

    match load_app_config() {
        Ok(config) => {
            let key = &config.ai_config.api_key;
            if key.is_empty() {
                findings.push(AuditFinding {
                    check_id: "api-key-missing".into(),
                    severity: Severity::Warning,
                    title: "API Key 未配置".into(),
                    detail: "AI 模型的 API Key 未设置，Agent 功能无法使用".into(),
                    remediation: Some("在设置 → AI 配置中填写 API Key".into()),
                });
            }

            // Check if API key looks like a test/demo key
            if key.starts_with("sk-demo") || key.starts_with("test-") || key == "your-api-key-here" {
                findings.push(AuditFinding {
                    check_id: "api-key-demo".into(),
                    severity: Severity::Critical,
                    title: "使用了测试/演示 API Key".into(),
                    detail: format!("API Key 以 '{}...' 开头，看起来是测试密钥", &key[..key.len().min(8)]),
                    remediation: Some("替换为有效的生产 API Key".into()),
                });
            }

            // Check base URL security
            let base_url = &config.ai_config.base_url;
            if base_url.starts_with("http://") && !base_url.contains("localhost") && !base_url.contains("127.0.0.1") {
                findings.push(AuditFinding {
                    check_id: "api-insecure-http".into(),
                    severity: Severity::Warning,
                    title: "API 使用不安全的 HTTP 连接".into(),
                    detail: format!("Base URL '{}' 使用明文 HTTP，API Key 可能被截获", base_url),
                    remediation: Some("改用 HTTPS URL".into()),
                });
            }

            // Check notifications config
            if let Some(ref notif) = config.notifications {
                if let Some(ref url) = notif.feishu_webhook {
                    if url.starts_with("http://") {
                        findings.push(AuditFinding {
                            check_id: "webhook-insecure-feishu".into(),
                            severity: Severity::Warning,
                            title: "飞书 Webhook 使用不安全连接".into(),
                            detail: "飞书 Webhook URL 使用明文 HTTP".into(),
                            remediation: Some("使用 HTTPS Webhook URL".into()),
                        });
                    }
                }
                if let Some(ref url) = notif.dingtalk_webhook {
                    if url.starts_with("http://") {
                        findings.push(AuditFinding {
                            check_id: "webhook-insecure-dingtalk".into(),
                            severity: Severity::Warning,
                            title: "钉钉 Webhook 使用不安全连接".into(),
                            detail: "钉钉 Webhook URL 使用明文 HTTP".into(),
                            remediation: Some("使用 HTTPS Webhook URL".into()),
                        });
                    }
                }
            }
        }
        Err(_) => {
            findings.push(AuditFinding {
                check_id: "config-load-fail".into(),
                severity: Severity::Critical,
                title: "配置文件加载失败".into(),
                detail: "无法读取应用配置文件".into(),
                remediation: Some("检查配置文件是否存在且格式正确".into()),
            });
        }
    }

    findings
}

#[cfg(unix)]
fn check_filesystem_permissions() -> Vec<AuditFinding> {
    let mut findings = Vec::new();

    if let Ok(data_dir) = get_data_dir() {
        // Check data directory permissions
        if let Ok(meta) = std::fs::metadata(&data_dir) {
            let mode = meta.permissions().mode();
            let world_readable = mode & 0o004 != 0;
            let world_writable = mode & 0o002 != 0;

            if world_writable {
                findings.push(AuditFinding {
                    check_id: "datadir-world-writable".into(),
                    severity: Severity::Critical,
                    title: "数据目录全局可写".into(),
                    detail: format!("数据目录 {:?} 权限为 {:o}，任何用户都可修改", data_dir, mode & 0o777),
                    remediation: Some(format!("运行: chmod 700 {:?}", data_dir)),
                });
            } else if world_readable {
                findings.push(AuditFinding {
                    check_id: "datadir-world-readable".into(),
                    severity: Severity::Warning,
                    title: "数据目录全局可读".into(),
                    detail: format!("数据目录 {:?} 权限为 {:o}，其他用户可读取", data_dir, mode & 0o777),
                    remediation: Some(format!("运行: chmod 700 {:?}", data_dir)),
                });
            }
        }

        // Check config file permissions
        let config_path = data_dir.join("config.json");
        if config_path.exists() {
            if let Ok(meta) = std::fs::metadata(&config_path) {
                let mode = meta.permissions().mode();
                if mode & 0o044 != 0 {
                    findings.push(AuditFinding {
                        check_id: "config-readable-others".into(),
                        severity: Severity::Warning,
                        title: "配置文件可被其他用户读取".into(),
                        detail: format!("配置文件权限为 {:o}，包含 API Key 等敏感信息", mode & 0o777),
                        remediation: Some(format!("运行: chmod 600 {:?}", config_path)),
                    });
                }
            }
        }

        // Check database permissions
        let db_path = data_dir.join("helix.db");
        if db_path.exists() {
            if let Ok(meta) = std::fs::metadata(&db_path) {
                let mode = meta.permissions().mode();
                if mode & 0o044 != 0 {
                    findings.push(AuditFinding {
                        check_id: "db-readable-others".into(),
                        severity: Severity::Warning,
                        title: "数据库文件可被其他用户读取".into(),
                        detail: format!("数据库 {:?} 权限为 {:o}，包含对话记录和密钥", db_path, mode & 0o777),
                        remediation: Some(format!("运行: chmod 600 {:?}", db_path)),
                    });
                }
            }
        }
    }

    findings
}

#[cfg(not(unix))]
fn check_filesystem_permissions() -> Vec<AuditFinding> {
    // Filesystem permission checks are Unix-only
    Vec::new()
}

#[cfg(unix)]
fn check_elevated_process() -> Vec<AuditFinding> {
    let mut findings = Vec::new();

    // Check if running as root
    let uid = unsafe { libc::getuid() };
    if uid == 0 {
        findings.push(AuditFinding {
            check_id: "running-as-root".into(),
            severity: Severity::Critical,
            title: "以 root 用户运行".into(),
            detail: "Helix 正在以 root 权限运行，Agent 执行的命令将拥有完全系统权限".into(),
            remediation: Some("以普通用户运行 Helix".into()),
        });
    }

    findings
}

#[cfg(not(unix))]
fn check_elevated_process() -> Vec<AuditFinding> {
    // Elevated process check is Unix-only
    Vec::new()
}

fn check_environment() -> Vec<AuditFinding> {
    let mut findings = Vec::new();

    // Check for debug mode
    if cfg!(debug_assertions) {
        findings.push(AuditFinding {
            check_id: "debug-build".into(),
            severity: Severity::Info,
            title: "调试构建".into(),
            detail: "当前为调试构建，性能较低".into(),
            remediation: Some("使用 release 构建部署".into()),
        });
    }

    // Check SSH agent
    if std::env::var("SSH_AUTH_SOCK").is_ok() {
        findings.push(AuditFinding {
            check_id: "ssh-agent-available".into(),
            severity: Severity::Info,
            title: "SSH Agent 已启用".into(),
            detail: "SSH Agent 正在运行，shell_exec 工具可以使用 SSH 密钥".into(),
            remediation: None,
        });
    }

    // Check for common dangerous env vars
    if std::env::var("SUDO_USER").is_ok() {
        findings.push(AuditFinding {
            check_id: "running-via-sudo".into(),
            severity: Severity::Warning,
            title: "通过 sudo 运行".into(),
            detail: "Helix 通过 sudo 启动，Agent 命令将以提升权限执行".into(),
            remediation: Some("不要使用 sudo 运行 Helix".into()),
        });
    }

    findings
}

// ============================================================================
// Public API
// ============================================================================

pub fn run_security_audit() -> AuditReport {
    let mut all_findings = Vec::new();

    all_findings.extend(check_api_key_security());
    all_findings.extend(check_filesystem_permissions());
    all_findings.extend(check_elevated_process());
    all_findings.extend(check_environment());

    let summary = AuditSummary {
        critical: all_findings.iter().filter(|f| f.severity == Severity::Critical).count(),
        warning: all_findings.iter().filter(|f| f.severity == Severity::Warning).count(),
        info: all_findings.iter().filter(|f| f.severity == Severity::Info).count(),
    };

    let data_dir = get_data_dir().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();

    info!(
        "Security audit complete: {} critical, {} warn, {} info",
        summary.critical, summary.warning, summary.info
    );

    AuditReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        summary,
        findings: all_findings,
        system_info: SystemInfo {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            user: std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default(),
            home: std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default(),
            data_dir,
        },
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn security_audit() -> Result<AuditReport, String> {
    Ok(run_security_audit())
}
