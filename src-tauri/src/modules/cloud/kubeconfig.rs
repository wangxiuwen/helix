use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Kubeconfig 集群信息（不含敏感证书数据）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubeCluster {
    pub name: String,
    pub server: String,
}

/// Kubeconfig Context 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubeContext {
    pub name: String,
    pub cluster: String,
    pub user: String,
    pub namespace: Option<String>,
}

/// 汇总的 Kubeconfig 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubeInfo {
    pub clusters: Vec<KubeCluster>,
    pub contexts: Vec<KubeContext>,
    pub current_context: Option<String>,
    pub config_path: String,
    pub config_exists: bool,
}

// ----- serde_yaml 解析用的内部结构 -----

#[derive(Deserialize)]
struct RawKubeConfig {
    clusters: Option<Vec<RawClusterEntry>>,
    contexts: Option<Vec<RawContextEntry>>,
    #[serde(rename = "current-context")]
    current_context: Option<String>,
}

#[derive(Deserialize)]
struct RawClusterEntry {
    name: String,
    cluster: Option<RawClusterData>,
}

#[derive(Deserialize)]
struct RawClusterData {
    server: Option<String>,
}

#[derive(Deserialize)]
struct RawContextEntry {
    name: String,
    context: Option<RawContextData>,
}

#[derive(Deserialize)]
struct RawContextData {
    cluster: Option<String>,
    user: Option<String>,
    namespace: Option<String>,
}

/// 获取 kubeconfig 文件路径
fn get_kubeconfig_path(custom_path: Option<&str>) -> PathBuf {
    if let Some(p) = custom_path {
        let expanded = if p.starts_with("~/") {
            dirs::home_dir()
                .map(|h| h.join(&p[2..]))
                .unwrap_or_else(|| PathBuf::from(p))
        } else {
            PathBuf::from(p)
        };
        return expanded;
    }

    // 检查 KUBECONFIG 环境变量
    if let Ok(env_path) = std::env::var("KUBECONFIG") {
        return PathBuf::from(env_path);
    }

    // 默认 ~/.kube/config
    dirs::home_dir()
        .map(|h| h.join(".kube").join("config"))
        .unwrap_or_else(|| PathBuf::from("~/.kube/config"))
}

/// 读取并解析 kubeconfig
pub fn load_kube_info(custom_path: Option<&str>) -> Result<KubeInfo, String> {
    let config_path = get_kubeconfig_path(custom_path);
    let path_str = config_path.display().to_string();

    if !config_path.exists() {
        return Ok(KubeInfo {
            clusters: vec![],
            contexts: vec![],
            current_context: None,
            config_path: path_str,
            config_exists: false,
        });
    }

    let content =
        std::fs::read_to_string(&config_path).map_err(|e| format!("读取 kubeconfig 失败: {}", e))?;

    let raw: RawKubeConfig =
        serde_yaml::from_str(&content).map_err(|e| format!("解析 kubeconfig 失败: {}", e))?;

    let clusters = raw
        .clusters
        .unwrap_or_default()
        .into_iter()
        .map(|c| KubeCluster {
            name: c.name,
            server: c
                .cluster
                .and_then(|d| d.server)
                .unwrap_or_else(|| "unknown".to_string()),
        })
        .collect();

    let contexts = raw
        .contexts
        .unwrap_or_default()
        .into_iter()
        .map(|c| {
            let ctx = c.context.unwrap_or(RawContextData {
                cluster: None,
                user: None,
                namespace: None,
            });
            KubeContext {
                name: c.name,
                cluster: ctx.cluster.unwrap_or_default(),
                user: ctx.user.unwrap_or_default(),
                namespace: ctx.namespace,
            }
        })
        .collect();

    Ok(KubeInfo {
        clusters,
        contexts,
        current_context: raw.current_context,
        config_path: path_str,
        config_exists: true,
    })
}
