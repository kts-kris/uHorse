//! # 密钥管理
//!
//! 安全存储和管理敏感配置。

use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};
use uhorse_core::Result;

/// 密钥存储
#[derive(Debug)]
pub struct SecretStore {
    secrets: HashMap<String, String>,
}

impl SecretStore {
    /// 创建新的密钥存储
    pub fn new() -> Self {
        Self {
            secrets: HashMap::new(),
        }
    }

    /// 从环境变量加载密钥
    pub fn load_from_env(&mut self, prefix: &str) -> Result<()> {
        info!("Loading secrets from environment with prefix: {}", prefix);

        for (key, value) in std::env::vars() {
            if key.starts_with(prefix) {
                let secret_key = key.strip_prefix(prefix).unwrap_or(&key);
                info!("Loaded secret: {}", secret_key);
                self.secrets.insert(secret_key.to_string(), value);
            }
        }

        Ok(())
    }

    /// 从配置文件加载密钥
    pub async fn load_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();

        // 检查文件是否存在
        if !path.exists() {
            warn!("Secret file not found: {}", path.display());
            return Ok(());
        }

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| uhorse_core::StorageError::ConnectionError(e.to_string()))?;

        // 简单的 key=value 格式
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                self.secrets
                    .insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        info!(
            "Loaded {} secrets from file: {}",
            self.secrets.len(),
            path.display()
        );

        Ok(())
    }

    /// 获取密钥
    pub fn get(&self, key: &str) -> Option<&String> {
        self.secrets.get(key)
    }

    /// 设置密钥
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.secrets.insert(key.into(), value.into());
    }

    /// 移除密钥
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.secrets.remove(key)
    }

    /// 列出所有密钥名称
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.secrets.keys()
    }
}

impl Default for SecretStore {
    fn default() -> Self {
        Self::new()
    }
}
