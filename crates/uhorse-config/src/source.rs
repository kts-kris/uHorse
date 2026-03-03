//! # 配置源
//!
//! 定义各种配置来源（文件、环境变量等）。

use anyhow::{Context, Result as AnyhowResult};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// 配置源 trait
pub trait ConfigSource: Send + Sync {
    /// 加载配置值
    fn load(&self) -> AnyhowResult<ConfigValue>;

    /// 源名称
    fn name(&self) -> &str {
        "unknown"
    }
}

/// 配置值
pub enum ConfigValue {
    Json(Value),
    Toml(String),
}

impl ConfigValue {
    /// 转换为 JSON
    pub fn as_json(&self) -> AnyhowResult<Value> {
        match self {
            ConfigValue::Json(v) => Ok(v.clone()),
            ConfigValue::Toml(s) => {
                let toml_value: toml::Value = toml::from_str(s).context("Failed to parse TOML")?;
                serde_json::to_value(toml_value).context("Failed to convert TOML to JSON")
            }
        }
    }
}

/// 合并策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// 默认：深度合并
    Default,
    /// 覆盖：完全使用 overlay
    Override,
}

/// 文件配置源
#[derive(Debug)]
pub struct FileSource {
    path: PathBuf,
}

impl FileSource {
    /// 创建新的文件源
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ConfigSource for FileSource {
    fn load(&self) -> AnyhowResult<ConfigValue> {
        let content = std::fs::read_to_string(&self.path)
            .with_context(|| format!("Failed to read config file: {:?}", self.path))?;

        let ext = self.path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "toml" => Ok(ConfigValue::Toml(content)),
            "json" => {
                let value: Value =
                    serde_json::from_str(&content).context("Failed to parse JSON")?;
                Ok(ConfigValue::Json(value))
            }
            _ => Ok(ConfigValue::Toml(content)), // 默认 TOML
        }
    }

    fn name(&self) -> &str {
        "file"
    }
}

/// 环境变量配置源
#[derive(Debug)]
pub struct EnvSource {
    prefix: String,
}

impl EnvSource {
    /// 创建新的环境变量源
    pub fn new() -> Self {
        Self {
            prefix: "OPENCLAW_".to_string(),
        }
    }

    /// 设置前缀
    pub fn with_prefix(mut self, prefix: String) -> Self {
        self.prefix = prefix;
        self
    }

    /// 将环境变量键转换为配置路径
    fn env_to_path(&self, key: &str) -> Option<Vec<String>> {
        if !key.starts_with(&self.prefix) {
            return None;
        }

        let path = key[self.prefix.len()..]
            .split('_')
            .map(|s| s.to_lowercase())
            .collect();

        Some(path)
    }
}

impl Default for EnvSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigSource for EnvSource {
    fn load(&self) -> AnyhowResult<ConfigValue> {
        let mut map = serde_json::Map::new();

        for (key, value) in std::env::vars() {
            if let Some(path) = self.env_to_path(&key) {
                let json_value =
                    serde_json::to_value(value).context("Failed to convert env value to JSON")?;
                map = insert_nested(map, &path, json_value);
            }
        }

        Ok(ConfigValue::Json(serde_json::Value::Object(map)))
    }

    fn name(&self) -> &str {
        "env"
    }
}

/// 在嵌套 Map 中插入值
fn insert_nested(
    mut map: serde_json::Map<String, Value>,
    path: &[String],
    value: Value,
) -> serde_json::Map<String, Value> {
    if path.len() == 1 {
        map.insert(path[0].clone(), value);
        map
    } else {
        let nested = map
            .remove(&path[0])
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        let new_nested = insert_nested(nested, &path[1..], value);
        map.insert(path[0].clone(), serde_json::Value::Object(new_nested));
        map
    }
}

/// 内存配置源
#[derive(Debug)]
pub struct MemorySource {
    data: HashMap<String, String>,
}

impl MemorySource {
    /// 创建新的内存源
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// 添加键值对
    pub fn set(mut self, key: String, value: String) -> Self {
        self.data.insert(key, value);
        self
    }
}

impl Default for MemorySource {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigSource for MemorySource {
    fn load(&self) -> AnyhowResult<ConfigValue> {
        let mut map = serde_json::Map::new();

        for (key, value) in &self.data {
            let path: Vec<String> = key.split('.').map(|s| s.to_string()).collect();
            let json_value =
                serde_json::to_value(value).context("Failed to convert memory value to JSON")?;
            map = insert_nested(map, &path, json_value);
        }

        Ok(ConfigValue::Json(serde_json::Value::Object(map)))
    }

    fn name(&self) -> &str {
        "memory"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_to_path() {
        let source = EnvSource::new();
        assert_eq!(
            source.env_to_path("OPENCLAW_SERVER_HOST"),
            Some(vec!["server".to_string(), "host".to_string()])
        );
        assert_eq!(source.env_to_path("OTHER_VAR"), None);
    }

    #[test]
    fn test_memory_source() {
        let source = MemorySource::new()
            .set("server.host".to_string(), "127.0.0.1".to_string())
            .set("server.port".to_string(), "3000".to_string());

        let value = source.load().unwrap();
        let json = value.as_json().unwrap();

        assert_eq!(json["server"]["host"], "127.0.0.1");
        assert_eq!(json["server"]["port"], "3000");
    }
}
