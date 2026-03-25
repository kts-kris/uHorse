//! Distributed configuration storage
//!
//! This module provides distributed configuration storage capabilities
//! for multi-node deployments.

use anyhow::Result;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Configuration key prefix
const CONFIG_PREFIX: &str = "/uhorse/config/";

/// Cached configuration entry
#[derive(Debug, Clone)]
struct CachedConfig {
    value: String,
}

/// Distributed configuration client
pub struct DistributedConfigClient {
    cache: Arc<RwLock<HashMap<String, CachedConfig>>>,
    backend: Arc<dyn ConfigBackend>,
}

impl DistributedConfigClient {
    /// Create a new distributed config client
    pub fn new(backend: Arc<dyn ConfigBackend>) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            backend,
        }
    }

    /// Create with in-memory backend
    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryConfigBackend::new()))
    }

    /// Get a configuration value
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let full_key = format!("{}{}", CONFIG_PREFIX, key);

        // Check cache first
        {
            let cache: tokio::sync::RwLockReadGuard<'_, HashMap<String, CachedConfig>> =
                self.cache.read().await;
            if let Some(cached) = cache.get(&full_key) {
                if let Ok(value) = serde_json::from_str::<T>(&cached.value) {
                    return Ok(Some(value));
                }
            }
        }

        // Try backend
        if let Some(value_str) = self.backend.get(&full_key).await? {
            let cached = CachedConfig {
                value: value_str.clone(),
            };

            // Update cache
            {
                let mut cache: tokio::sync::RwLockWriteGuard<'_, HashMap<String, CachedConfig>> =
                    self.cache.write().await;
                cache.insert(full_key.clone(), cached);
            }

            let value: T = serde_json::from_str(&value_str)?;
            return Ok(Some(value));
        }

        Ok(None)
    }

    /// Set a configuration value
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let full_key = format!("{}{}", CONFIG_PREFIX, key);
        let value_str = serde_json::to_string(value)?;

        // Update backend
        self.backend.set(&full_key, &value_str).await?;

        // Update cache
        {
            let mut cache: tokio::sync::RwLockWriteGuard<'_, HashMap<String, CachedConfig>> =
                self.cache.write().await;
            cache.insert(full_key, CachedConfig { value: value_str });
        }

        tracing::info!("Set config key: {}", key);
        Ok(())
    }

    /// Delete a configuration value
    pub async fn delete(&self, key: &str) -> Result<()> {
        let full_key = format!("{}{}", CONFIG_PREFIX, key);

        // Delete from backend
        self.backend.delete(&full_key).await?;

        // Update cache
        {
            let mut cache: tokio::sync::RwLockWriteGuard<'_, HashMap<String, CachedConfig>> =
                self.cache.write().await;
            cache.remove(&full_key);
        }

        tracing::info!("Deleted config key: {}", key);
        Ok(())
    }

    /// List all configuration keys
    pub async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let full_prefix = format!("{}{}", CONFIG_PREFIX, prefix);
        let keys: Vec<String> = self.backend.list(&full_prefix).await?;

        Ok(keys
            .into_iter()
            .filter_map(|k: String| k.strip_prefix(CONFIG_PREFIX).map(|s| s.to_string()))
            .collect())
    }

    /// Check if distributed storage is available
    pub fn is_distributed(&self) -> bool {
        true // Always true since we have a backend
    }
}

/// Options for distributed configuration
#[derive(Debug, Clone)]
pub struct DistributedConfigOptions {
    /// Cache TTL in seconds
    pub cache_ttl_secs: u64,
    /// Enable automatic refresh
    pub auto_refresh: bool,
}

impl Default for DistributedConfigOptions {
    fn default() -> Self {
        Self {
            cache_ttl_secs: 300, // 5 minutes
            auto_refresh: true,
        }
    }
}

/// Configuration backend trait
#[async_trait]
pub trait ConfigBackend: Send + Sync {
    /// Get a configuration value
    async fn get(&self, key: &str) -> Result<Option<String>>;

    /// Set a configuration value
    async fn set(&self, key: &str, value: &str) -> Result<()>;

    /// Delete a configuration value
    async fn delete(&self, key: &str) -> Result<()>;

    /// List all keys with a prefix
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;

    /// Watch for changes on a key or prefix
    async fn watch(&self, key: &str) -> Result<ConfigWatchStream>;
}

/// Watch stream type
pub type ConfigWatchStream = mpsc::Receiver<ConfigWatchEvent>;

/// Configuration watch event
#[derive(Debug, Clone)]
pub enum ConfigWatchEvent {
    /// Key was created or updated
    Put { key: String, value: String },
    /// Key was deleted
    Delete { key: String },
}

/// In-memory backend for local development
pub struct InMemoryConfigBackend {
    data: Arc<RwLock<HashMap<String, String>>>,
}

impl InMemoryConfigBackend {
    /// Create a new in-memory backend
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryConfigBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConfigBackend for InMemoryConfigBackend {
    async fn get(&self, key: &str) -> Result<Option<String>> {
        let data: tokio::sync::RwLockReadGuard<'_, HashMap<String, String>> =
            self.data.read().await;
        Ok(data.get(key).cloned())
    }

    async fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut data: tokio::sync::RwLockWriteGuard<'_, HashMap<String, String>> =
            self.data.write().await;
        data.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut data: tokio::sync::RwLockWriteGuard<'_, HashMap<String, String>> =
            self.data.write().await;
        data.remove(key);
        Ok(())
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let data: tokio::sync::RwLockReadGuard<'_, HashMap<String, String>> =
            self.data.read().await;
        let keys: Vec<String> = data
            .keys()
            .filter(|k: &&String| k.starts_with(prefix))
            .cloned()
            .collect();
        Ok(keys)
    }

    async fn watch(&self, _key: &str) -> Result<ConfigWatchStream> {
        let (tx, rx) = mpsc::channel(100);
        let _ = tx; // Suppress unused warning
        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_backend() {
        let backend = InMemoryConfigBackend::new();

        // Test set and get
        backend.set("test.key", "test_value").await.unwrap();
        let value = backend.get("test.key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        // Test delete
        backend.delete("test.key").await.unwrap();
        let value = backend.get("test.key").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_list_keys() {
        let backend = InMemoryConfigBackend::new();

        backend.set("app.server.host", "localhost").await.unwrap();
        backend.set("app.server.port", "8765").await.unwrap();
        backend
            .set("app.database.url", "sqlite://db")
            .await
            .unwrap();

        let keys = backend.list("app.").await.unwrap();
        assert_eq!(keys.len(), 3);
    }

    #[tokio::test]
    async fn test_distributed_client() {
        let client = DistributedConfigClient::in_memory();

        // Set and get
        client
            .set("test.key", &"test_value".to_string())
            .await
            .unwrap();
        let value: Option<String> = client.get("test.key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));
    }
}
