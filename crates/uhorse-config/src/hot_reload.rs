//! Configuration hot reload mechanism

use anyhow::Result;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

type ConfigReloaders = Vec<Arc<dyn ConfigReloader>>;
type ConfigSubscriberMap = HashMap<String, ConfigReloaders>;

/// Configuration change event
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    /// Key that changed
    pub key: String,
    /// Old value (None if created)
    pub old_value: Option<String>,
    /// New value (None if deleted)
    pub new_value: Option<String>,
}

/// Configuration reloader trait
#[async_trait]
pub trait ConfigReloader: Send + Sync {
    /// Called when configuration changes
    async fn on_change(&self, event: &ConfigChangeEvent) -> Result<()>;
}

/// Hot reload manager
pub struct HotReloadManager {
    subscribers: Arc<RwLock<ConfigSubscriberMap>>,
    tx: broadcast::Sender<ConfigChangeEvent>,
}

impl HotReloadManager {
    /// Create a new hot reload manager
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            tx,
        }
    }

    /// Subscribe to configuration changes for a key pattern
    pub async fn subscribe(&self, key_pattern: &str, reloader: Arc<dyn ConfigReloader>) {
        let mut subscribers = self.subscribers.write().await;
        subscribers
            .entry(key_pattern.to_string())
            .or_default()
            .push(reloader);

        info!("Subscribed to config changes for pattern: {}", key_pattern);
    }

    /// Notify subscribers of a configuration change
    pub async fn notify(&self, event: ConfigChangeEvent) -> Result<()> {
        // Broadcast to all listeners
        let _ = self.tx.send(event.clone());

        // Notify pattern-matched subscribers
        let subscribers = self.subscribers.read().await;
        for (pattern, reloaders) in subscribers.iter() {
            if matches_pattern(pattern, &event.key) {
                for reloader in reloaders {
                    if let Err(e) = reloader.on_change(&event).await {
                        warn!("Config reload failed for {}: {}", pattern, e);
                    }
                }
            }
        }

        info!("Notified config change for key: {}", event.key);
        Ok(())
    }

    /// Get a receiver for configuration change events
    pub fn subscribe_channel(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.tx.subscribe()
    }
}

impl Default for HotReloadManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a key matches a pattern
fn matches_pattern(pattern: &str, key: &str) -> bool {
    if pattern == "*" || pattern == "**" {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
        return key.starts_with(prefix);
    }

    pattern == key
}

/// Typed configuration watcher
pub struct ConfigWatcher<T: Clone + DeserializeOwned + Send + Sync + 'static> {
    key: String,
    current: Arc<RwLock<Option<T>>>,
}

impl<T: Clone + DeserializeOwned + Send + Sync + 'static> ConfigWatcher<T> {
    /// Create a new watcher for a configuration key
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            current: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the current value
    pub async fn get(&self) -> Option<T> {
        let current = self.current.read().await;
        current.clone()
    }

    /// Update the value
    pub async fn update(&self, value: T) {
        let mut current = self.current.write().await;
        *current = Some(value);
    }

    /// Get the watched key
    pub fn key(&self) -> &str {
        &self.key
    }
}

/// Builder for hot reload configuration
pub struct HotReloadBuilder {
    manager: HotReloadManager,
}

impl HotReloadBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            manager: HotReloadManager::new(),
        }
    }

    /// Add a watcher for a configuration key
    pub async fn watch<F>(self, key: &str, handler: F) -> Self
    where
        F: Fn(ConfigChangeEvent) -> Result<()> + Send + Sync + 'static,
    {
        let reloader = Arc::new(ClosureReloader { handler });
        self.manager.subscribe(key, reloader).await;
        self
    }

    /// Build the manager
    pub fn build(self) -> HotReloadManager {
        self.manager
    }
}

impl Default for HotReloadBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Closure-based reloader
struct ClosureReloader<F> {
    handler: F,
}

#[async_trait]
impl<F> ConfigReloader for ClosureReloader<F>
where
    F: Fn(ConfigChangeEvent) -> Result<()> + Send + Sync,
{
    async fn on_change(&self, event: &ConfigChangeEvent) -> Result<()> {
        (self.handler)(event.clone())
    }
}

/// Reloadable configuration wrapper
pub struct ReloadableConfig<T: Clone + Send + Sync + 'static> {
    inner: Arc<RwLock<T>>,
    key: String,
}

impl<T: Clone + Send + Sync + 'static> ReloadableConfig<T> {
    /// Create a new reloadable config
    pub fn new(key: impl Into<String>, initial: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial)),
            key: key.into(),
        }
    }

    /// Get the current value
    pub async fn get(&self) -> T {
        self.inner.read().await.clone()
    }

    /// Update the value
    pub async fn set(&self, value: T) {
        let mut inner = self.inner.write().await;
        *inner = value;
    }

    /// Get the config key
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl<T: Clone + Send + Sync + 'static> Clone for ReloadableConfig<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            key: self.key.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("*", "any.key"));
        assert!(matches_pattern("**", "any.key"));
        assert!(matches_pattern("app.*", "app.server"));
        assert!(matches_pattern("app.*", "app.database"));
        assert!(!matches_pattern("app.*", "other.key"));
        assert!(matches_pattern("app.server", "app.server"));
        assert!(!matches_pattern("app.server", "app.database"));
    }

    #[tokio::test]
    async fn test_reloadable_config() {
        let config = ReloadableConfig::new("test.key", "initial".to_string());
        assert_eq!(config.get().await, "initial");

        config.set("updated".to_string()).await;
        assert_eq!(config.get().await, "updated");
    }

    #[tokio::test]
    async fn test_hot_reload_manager() {
        let manager = HotReloadManager::new();

        let event = ConfigChangeEvent {
            key: "test.key".to_string(),
            old_value: Some("old".to_string()),
            new_value: Some("new".to_string()),
        };

        // Should not error even with no subscribers
        manager.notify(event).await.unwrap();
    }
}
