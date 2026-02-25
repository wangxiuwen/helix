use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 阿里云 Profile 信息（脱敏后）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliyunProfile {
    pub name: String,
    pub mode: String,
    pub access_key_hint: String, // 只显示尾部 4 位
    pub region_id: String,
}

/// 汇总的阿里云配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliyunInfo {
    pub profiles: Vec<AliyunProfile>,
    pub current: Option<String>,
    pub config_path: String,
    pub config_exists: bool,
}

// ----- JSON 解析用的内部结构 -----

#[derive(Deserialize)]
struct RawAliyunConfig {
    current: Option<String>,
    profiles: Option<Vec<RawAliyunProfile>>,
}

#[derive(Deserialize)]
struct RawAliyunProfile {
    name: Option<String>,
    mode: Option<String>,
    access_key_id: Option<String>,
    region_id: Option<String>,
}

/// 获取阿里云配置文件路径
fn get_aliyun_config_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".aliyun").join("config.json"))
        .unwrap_or_else(|| PathBuf::from("~/.aliyun/config.json"))
}

/// 对 AccessKey ID 做脱敏处理：只保留末尾 4 位
fn mask_key(key: &str) -> String {
    if key.len() <= 4 {
        return "****".to_string();
    }
    format!("****{}", &key[key.len() - 4..])
}

/// 读取并解析阿里云配置
pub fn load_aliyun_info() -> Result<AliyunInfo, String> {
    let config_path = get_aliyun_config_path();
    let path_str = config_path.display().to_string();

    if !config_path.exists() {
        return Ok(AliyunInfo {
            profiles: vec![],
            current: None,
            config_path: path_str,
            config_exists: false,
        });
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("读取阿里云配置失败: {}", e))?;

    let raw: RawAliyunConfig =
        serde_json::from_str(&content).map_err(|e| format!("解析阿里云配置失败: {}", e))?;

    let profiles = raw
        .profiles
        .unwrap_or_default()
        .into_iter()
        .map(|p| AliyunProfile {
            name: p.name.unwrap_or_else(|| "unnamed".to_string()),
            mode: p.mode.unwrap_or_else(|| "AK".to_string()),
            access_key_hint: p
                .access_key_id
                .as_deref()
                .map(mask_key)
                .unwrap_or_else(|| "未配置".to_string()),
            region_id: p.region_id.unwrap_or_else(|| "cn-beijing".to_string()),
        })
        .collect();

    Ok(AliyunInfo {
        profiles,
        current: raw.current,
        config_path: path_str,
        config_exists: true,
    })
}
