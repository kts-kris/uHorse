//! # 幂等性缓存
//!
//! 保证请求的幂等性。

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uhorse_core::{IdempotencyService, Result};

/// 幂等性记录
#[derive(Debug, Clone)]
struct IdempotencyRecord {
    response: serde_json::Value,
    expires_at: u64,
}

/// 幂等性缓存
#[derive(Debug)]
pub struct IdempotencyCache {
    records: Arc<RwLock<HashMap<String, IdempotencyRecord>>>,
}

impl IdempotencyCache {
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for IdempotencyCache {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl IdempotencyService for IdempotencyCache {
    async fn check_or_record(
        &self,
        key: &str,
        ttl_seconds: u64,
    ) -> Result<Option<serde_json::Value>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut records = self.records.write().await;

        // 清理过期记录
        records.retain(|_, record| record.expires_at > now);

        if let Some(record) = records.get(key) {
            return Ok(Some(record.response.clone()));
        }

        Ok(None)
    }

    async fn store_response(
        &self,
        key: &str,
        response: &serde_json::Value,
        ttl_seconds: u64,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let record = IdempotencyRecord {
            response: response.clone(),
            expires_at: now + ttl_seconds,
        };

        self.records.write().await.insert(key.to_string(), record);
        Ok(())
    }

    async fn cleanup_expired(&self) -> Result<usize> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut records = self.records.write().await;
        let before = records.len();
        records.retain(|_, record| record.expires_at > now);
        Ok(before - records.len())
    }
}
