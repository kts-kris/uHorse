//! Configuration versioning and history management

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Maximum history entries per key
const MAX_HISTORY_SIZE: usize = 100;

/// Configuration version entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigVersion {
    /// Version number (incremental)
    pub version: u64,
    /// Configuration key
    pub key: String,
    /// Configuration value
    pub value: String,
    /// Timestamp of the change
    pub timestamp: DateTime<Utc>,
    /// User who made the change
    pub changed_by: Option<String>,
    /// Description of the change
    pub description: Option<String>,
}

/// Configuration history manager
pub struct ConfigHistory {
    history: Arc<RwLock<HashMap<String, VecDeque<ConfigVersion>>>>,
    current_versions: Arc<RwLock<HashMap<String, u64>>>,
    max_history: usize,
}

impl ConfigHistory {
    /// Create a new configuration history manager
    pub fn new() -> Self {
        Self::with_max_history(MAX_HISTORY_SIZE)
    }

    /// Create with custom max history size
    pub fn with_max_history(max_history: usize) -> Self {
        Self {
            history: Arc::new(RwLock::new(HashMap::new())),
            current_versions: Arc::new(RwLock::new(HashMap::new())),
            max_history,
        }
    }

    /// Record a configuration change
    pub async fn record(
        &self,
        key: impl Into<String>,
        value: impl Into<String>,
        changed_by: Option<String>,
        description: Option<String>,
    ) -> Result<ConfigVersion> {
        let key = key.into();
        let value = value.into();

        // Get next version number
        let version = {
            let mut versions = self.current_versions.write().await;
            let current = versions.entry(key.clone()).or_insert(0);
            *current += 1;
            *current
        };

        let version_entry = ConfigVersion {
            version,
            key: key.clone(),
            value,
            timestamp: Utc::now(),
            changed_by,
            description,
        };

        // Add to history
        {
            let mut history = self.history.write().await;
            let key_history = history.entry(key.clone()).or_default();
            key_history.push_back(version_entry.clone());

            // Trim old entries
            while key_history.len() > self.max_history {
                key_history.pop_front();
            }
        }

        tracing::info!("Recorded config version {} for key {}", version, key);
        Ok(version_entry)
    }

    /// Get history for a key
    pub async fn get_history(&self, key: &str) -> Vec<ConfigVersion> {
        let history = self.history.read().await;
        history
            .get(key)
            .map(|h| h.iter().rev().cloned().collect())
            .unwrap_or_default()
    }

    /// Get a specific version
    pub async fn get_version(&self, key: &str, version: u64) -> Option<ConfigVersion> {
        let history = self.history.read().await;
        history
            .get(key)
            .and_then(|h| h.iter().rev().find(|v| v.version == version).cloned())
    }

    /// Get current version number
    pub async fn current_version(&self, key: &str) -> u64 {
        let versions = self.current_versions.read().await;
        versions.get(key).copied().unwrap_or(0)
    }

    /// List all versioned keys
    pub async fn list_keys(&self) -> Vec<String> {
        let history = self.history.read().await;
        history.keys().cloned().collect()
    }

    /// Get recent changes across all keys
    pub async fn recent_changes(&self, limit: usize) -> Vec<ConfigVersion> {
        let history = self.history.read().await;
        let mut all_versions: Vec<ConfigVersion> = history
            .values()
            .flat_map(|h| h.iter().rev().cloned())
            .collect();

        // Sort by timestamp descending
        all_versions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        all_versions.truncate(limit);
        all_versions
    }

    /// Clear history for a key
    pub async fn clear_history(&self, key: &str) {
        let mut history = self.history.write().await;
        history.remove(key);

        let mut versions = self.current_versions.write().await;
        versions.remove(key);

        tracing::info!("Cleared config history for key {}", key);
    }
}

impl Default for ConfigHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration rollback manager
pub struct ConfigRollback {
    history: Arc<ConfigHistory>,
}

impl ConfigRollback {
    /// Create a new rollback manager
    pub fn new(history: Arc<ConfigHistory>) -> Self {
        Self { history }
    }

    /// Rollback to a specific version
    pub async fn rollback_to(&self, key: &str, version: u64) -> Result<String> {
        let version_entry = self
            .history
            .get_version(key, version)
            .await
            .ok_or_else(|| anyhow!("Version {} not found for key {}", version, key))?;

        // Record this as a new change (rollback)
        self.history
            .record(
                key,
                version_entry.value.clone(),
                version_entry.changed_by.clone(),
                Some(format!("Rollback to version {}", version)),
            )
            .await?;

        tracing::info!("Rolled back {} to version {}", key, version);
        Ok(version_entry.value)
    }

    /// Rollback to previous version
    pub async fn rollback_previous(&self, key: &str) -> Result<String> {
        let current = self.history.current_version(key).await;
        if current <= 1 {
            return Err(anyhow!("No previous version to rollback to"));
        }

        self.rollback_to(key, current - 1).await
    }

    /// Preview rollback (get version without applying)
    pub async fn preview_rollback(&self, key: &str, version: u64) -> Option<ConfigVersion> {
        self.history.get_version(key, version).await
    }
}

/// Configuration diff for comparing versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDiff {
    /// Key being compared
    pub key: String,
    /// Old version
    pub old_version: u64,
    /// New version
    pub new_version: u64,
    /// Old value
    pub old_value: String,
    /// New value
    pub new_value: String,
}

impl ConfigDiff {
    /// Create a diff between two versions
    pub fn new(key: String, old: ConfigVersion, new: ConfigVersion) -> Self {
        Self {
            key,
            old_version: old.version,
            new_version: new.version,
            old_value: old.value,
            new_value: new.value,
        }
    }

    /// Check if values are different
    pub fn has_changes(&self) -> bool {
        self.old_value != self.new_value
    }

    /// Get line-by-line diff
    pub fn line_diff(&self) -> Vec<DiffLine> {
        let old_lines: Vec<&str> = self.old_value.lines().collect();
        let new_lines: Vec<&str> = self.new_value.lines().collect();

        let mut result = Vec::new();
        let mut old_iter = old_lines.iter().peekable();
        let mut new_iter = new_lines.iter().peekable();

        loop {
            match (old_iter.peek(), new_iter.peek()) {
                (Some(old), Some(new)) if old == new => {
                    result.push(DiffLine::Unchanged(old_iter.next().unwrap().to_string()));
                    new_iter.next();
                }
                (Some(_), Some(_)) => {
                    // Simple diff: show old as removed, new as added
                    if let Some(old) = old_iter.next() {
                        result.push(DiffLine::Removed(old.to_string()));
                    }
                    if let Some(new) = new_iter.next() {
                        result.push(DiffLine::Added(new.to_string()));
                    }
                }
                (Some(_), None) => {
                    result.push(DiffLine::Removed(old_iter.next().unwrap().to_string()));
                }
                (None, Some(_)) => {
                    result.push(DiffLine::Added(new_iter.next().unwrap().to_string()));
                }
                (None, None) => break,
            }
        }

        result
    }
}

/// A single line in a diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiffLine {
    /// Unchanged line
    Unchanged(String),
    /// Added line
    Added(String),
    /// Removed line
    Removed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_and_retrieve() {
        let history = ConfigHistory::new();

        // Record some changes
        history
            .record("test.key", "value1", None, None)
            .await
            .unwrap();
        history
            .record("test.key", "value2", None, None)
            .await
            .unwrap();
        history
            .record("test.key", "value3", None, None)
            .await
            .unwrap();

        // Check current version
        assert_eq!(history.current_version("test.key").await, 3);

        // Get history
        let versions = history.get_history("test.key").await;
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].version, 3);
        assert_eq!(versions[0].value, "value3");
    }

    #[tokio::test]
    async fn test_rollback() {
        let history = Arc::new(ConfigHistory::new());

        history
            .record("test.key", "original", None, None)
            .await
            .unwrap();
        history
            .record("test.key", "modified", None, None)
            .await
            .unwrap();

        let rollback = ConfigRollback::new(history);

        // Rollback to version 1
        let value = rollback.rollback_to("test.key", 1).await.unwrap();
        assert_eq!(value, "original");
    }

    #[tokio::test]
    async fn test_config_diff() {
        let old = ConfigVersion {
            version: 1,
            key: "test".to_string(),
            value: "line1\nline2\nline3".to_string(),
            timestamp: Utc::now(),
            changed_by: None,
            description: None,
        };

        let new = ConfigVersion {
            version: 2,
            key: "test".to_string(),
            value: "line1\nline2_modified\nline3\nline4".to_string(),
            timestamp: Utc::now(),
            changed_by: None,
            description: None,
        };

        let diff = ConfigDiff::new("test".to_string(), old, new);
        assert!(diff.has_changes());

        let lines = diff.line_diff();
        assert!(!lines.is_empty());
    }

    #[tokio::test]
    async fn test_max_history() {
        let history = ConfigHistory::with_max_history(5);

        // Record more than max history
        for i in 0..10 {
            history
                .record("test.key", format!("value{}", i), None, None)
                .await
                .unwrap();
        }

        // Should only keep last 5
        let versions = history.get_history("test.key").await;
        assert_eq!(versions.len(), 5);
    }
}
