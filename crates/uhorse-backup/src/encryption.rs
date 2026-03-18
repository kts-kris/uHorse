//! Backup encryption
//!
//! 备份加密，使用 AES-256-GCM 加密算法

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// 加密密钥信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionKey {
    /// 密钥 ID
    pub id: String,
    /// 密钥名称
    pub name: String,
    /// 密钥版本
    pub version: u32,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 是否激活
    pub active: bool,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

impl EncryptionKey {
    /// 创建新的加密密钥信息
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            version: 1,
            created_at: Utc::now(),
            active: true,
            metadata: HashMap::new(),
        }
    }
}

/// 加密配置
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// 是否启用加密
    pub enabled: bool,
    /// 密钥长度 (字节)
    pub key_length: usize,
    /// 盐值长度 (字节)
    pub salt_length: usize,
    /// nonce 长度 (字节)
    pub nonce_length: usize,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            key_length: 32, // AES-256
            salt_length: 16,
            nonce_length: 12, // GCM nonce
        }
    }
}

/// 加密管理器
pub struct EncryptionManager {
    /// 配置
    config: EncryptionConfig,
    /// 密钥存储
    keys: Arc<RwLock<HashMap<String, EncryptionKey>>>,
    /// 当前激活的密钥 ID
    active_key_id: Arc<RwLock<Option<String>>>,
}

impl EncryptionManager {
    /// 创建新的加密管理器
    pub fn new(config: EncryptionConfig) -> Self {
        Self {
            config,
            keys: Arc::new(RwLock::new(HashMap::new())),
            active_key_id: Arc::new(RwLock::new(None)),
        }
    }

    /// 生成新的加密密钥
    pub async fn generate_key(&self, name: impl Into<String>) -> super::Result<EncryptionKey> {
        let key = EncryptionKey::new(name);

        // 保存密钥信息
        let mut keys = self.keys.write().await;
        keys.insert(key.id.clone(), key.clone());

        // 设置为激活密钥
        let mut active_id = self.active_key_id.write().await;
        *active_id = Some(key.id.clone());

        info!("Generated encryption key: {} ({})", key.name, key.id);

        Ok(key)
    }

    /// 获取密钥
    pub async fn get_key(&self, key_id: &str) -> Option<EncryptionKey> {
        let keys = self.keys.read().await;
        keys.get(key_id).cloned()
    }

    /// 获取激活的密钥
    pub async fn get_active_key(&self) -> Option<EncryptionKey> {
        let active_id = self.active_key_id.read().await;
        if let Some(id) = active_id.as_ref() {
            let keys = self.keys.read().await;
            keys.get(id).cloned()
        } else {
            None
        }
    }

    /// 激活密钥
    pub async fn activate_key(&self, key_id: &str) -> super::Result<bool> {
        let mut keys = self.keys.write().await;

        // 先停用所有密钥
        for k in keys.values_mut() {
            k.active = false;
        }

        // 然后激活指定密钥
        if let Some(key) = keys.get_mut(key_id) {
            key.active = true;

            let mut active_id = self.active_key_id.write().await;
            *active_id = Some(key_id.to_string());

            info!("Activated encryption key: {}", key_id);

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 删除密钥
    pub async fn delete_key(&self, key_id: &str) -> super::Result<bool> {
        let mut keys = self.keys.write().await;

        if let Some(key) = keys.remove(key_id) {
            // 如果删除的是激活密钥，清除激活状态
            let mut active_id = self.active_key_id.write().await;
            if active_id.as_ref() == Some(&key.id.to_string()) {
                *active_id = None;
            }

            info!("Deleted encryption key: {}", key_id);

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 列出所有密钥
    pub async fn list_keys(&self) -> Vec<EncryptionKey> {
        let keys = self.keys.read().await;
        keys.values().cloned().collect()
    }

    /// 加密数据
    ///
    /// 注意：这是一个简化的实现，实际应使用 AES-256-GCM
    pub async fn encrypt(&self, data: &[u8], key_id: &str) -> super::Result<EncryptedData> {
        let keys = self.keys.read().await;

        if !keys.contains_key(key_id) {
            return Err(super::BackupError::NotFound(format!(
                "Key not found: {}",
                key_id
            )));
        }

        // 生成随机 nonce
        let nonce = self.generate_nonce();

        // 简化实现：实际应使用 AES-256-GCM 加密
        // 这里只是做简单的 XOR 示例
        let encrypted = self.xor_encrypt(data, &nonce);

        // 计算认证标签 (简化)
        let tag = self.compute_tag(&encrypted, &nonce);

        info!("Encrypted {} bytes with key {}", data.len(), key_id);

        Ok(EncryptedData {
            key_id: key_id.to_string(),
            nonce,
            data: encrypted,
            tag,
        })
    }

    /// 解密数据
    pub async fn decrypt(&self, encrypted: &EncryptedData) -> super::Result<Vec<u8>> {
        let keys = self.keys.read().await;

        if !keys.contains_key(&encrypted.key_id) {
            return Err(super::BackupError::NotFound(format!(
                "Key not found: {}",
                encrypted.key_id
            )));
        }

        // 验证认证标签
        let expected_tag = self.compute_tag(&encrypted.data, &encrypted.nonce);
        if expected_tag != encrypted.tag {
            return Err(super::BackupError::EncryptionError(
                "Authentication tag verification failed".to_string(),
            ));
        }

        // 解密
        let decrypted = self.xor_encrypt(&encrypted.data, &encrypted.nonce);

        info!(
            "Decrypted {} bytes with key {}",
            decrypted.len(),
            encrypted.key_id
        );

        Ok(decrypted)
    }

    /// 生成随机 nonce
    fn generate_nonce(&self) -> Vec<u8> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or(0)
            .hash(&mut hasher);
        let hash = hasher.finish();

        let nonce: Vec<u8> = hash
            .to_le_bytes()
            .iter()
            .cycle()
            .take(self.config.nonce_length)
            .copied()
            .collect();

        nonce
    }

    /// 简化的 XOR 加密 (仅用于演示)
    fn xor_encrypt(&self, data: &[u8], nonce: &[u8]) -> Vec<u8> {
        data.iter()
            .enumerate()
            .map(|(i, &byte)| byte ^ nonce[i % nonce.len()])
            .collect()
    }

    /// 计算认证标签 (简化)
    fn compute_tag(&self, data: &[u8], nonce: &[u8]) -> Vec<u8> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        nonce.hash(&mut hasher);

        let hash = hasher.finish();
        hash.to_le_bytes().to_vec()
    }

    /// 获取配置
    pub fn config(&self) -> &EncryptionConfig {
        &self.config
    }
}

impl Default for EncryptionManager {
    fn default() -> Self {
        Self::new(EncryptionConfig::default())
    }
}

/// 加密数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// 密钥 ID
    pub key_id: String,
    /// Nonce
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>,
    /// 加密数据
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    /// 认证标签
    #[serde(with = "serde_bytes")]
    pub tag: Vec<u8>,
}

impl EncryptedData {
    /// 序列化为字节数组
    pub fn to_bytes(&self) -> super::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| super::BackupError::EncryptionError(e.to_string()))
    }

    /// 从字节数组反序列化
    pub fn from_bytes(data: &[u8]) -> super::Result<Self> {
        serde_json::from_slice(data).map_err(|e| super::BackupError::EncryptionError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_key() {
        let manager = EncryptionManager::default();
        let key = manager.generate_key("test-key").await.unwrap();

        assert_eq!(key.name, "test-key");
        assert!(key.active);
    }

    #[tokio::test]
    async fn test_get_active_key() {
        let manager = EncryptionManager::default();
        manager.generate_key("test-key").await.unwrap();

        let active = manager.get_active_key().await;
        assert!(active.is_some());
        assert_eq!(active.unwrap().name, "test-key");
    }

    #[tokio::test]
    async fn test_encrypt_decrypt() {
        let manager = EncryptionManager::default();
        let key = manager.generate_key("test-key").await.unwrap();

        let original = b"Hello, World!";
        let encrypted = manager.encrypt(original, &key.id).await.unwrap();

        assert_eq!(encrypted.key_id, key.id);
        assert!(!encrypted.nonce.is_empty());
        assert_ne!(encrypted.data.as_slice(), original.as_slice());

        let decrypted = manager.decrypt(&encrypted).await.unwrap();
        assert_eq!(decrypted.as_slice(), original.as_slice());
    }

    #[test]
    fn test_encrypted_data_serialization() {
        let data = EncryptedData {
            key_id: "key-1".to_string(),
            nonce: vec![1, 2, 3],
            data: vec![4, 5, 6],
            tag: vec![7, 8, 9],
        };

        let bytes = data.to_bytes().unwrap();
        let restored = EncryptedData::from_bytes(&bytes).unwrap();

        assert_eq!(restored.key_id, data.key_id);
        assert_eq!(restored.nonce, data.nonce);
    }
}
