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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_or_record_returns_none_for_new_key() {
        let cache = IdempotencyCache::new();
        let result = cache.check_or_record("new-key", 60).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_store_and_retrieve_response() {
        let cache = IdempotencyCache::new();
        let key = "test-key";
        let response = serde_json::json!({ "status": "success" });

        // 存储响应
        cache
            .store_response(key, &response, 60)
            .await
            .expect("Failed to store response");

        // 检索响应
        let result = cache.check_or_record(key, 60).await;
        assert!(result.is_ok());
        let retrieved = result.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), response);
    }

    #[tokio::test]
    async fn test_idempotency_prevents_duplicate_processing() {
        let cache = IdempotencyCache::new();
        let key = "idempotent-request";

        // 第一次检查应该返回 None
        let first_check = cache.check_or_record(key, 60).await.unwrap();
        assert!(first_check.is_none());

        // 存储响应
        let response = serde_json::json!({ "id": 123, "created": true });
        cache.store_response(key, &response, 60).await.unwrap();

        // 第二次检查应该返回存储的响应
        let second_check = cache.check_or_record(key, 60).await.unwrap();
        assert!(second_check.is_some());
        assert_eq!(second_check.unwrap(), response);
    }

    #[tokio::test]
    async fn test_different_keys_have_independent_records() {
        let cache = IdempotencyCache::new();

        let response1 = serde_json::json!({ "data": "first" });
        let response2 = serde_json::json!({ "data": "second" });

        cache.store_response("key1", &response1, 60).await.unwrap();
        cache.store_response("key2", &response2, 60).await.unwrap();

        let result1 = cache.check_or_record("key1", 60).await.unwrap().unwrap();
        let result2 = cache.check_or_record("key2", 60).await.unwrap().unwrap();

        assert_eq!(result1, response1);
        assert_eq!(result2, response2);
        assert_ne!(result1, result2);
    }

    #[tokio::test]
    async fn test_cleanup_expired_removes_old_records() {
        let cache = IdempotencyCache::new();

        // 存储一个马上过期的记录（TTL = 0）
        cache
            .store_response("expiring", &serde_json::json!({}), 0)
            .await
            .unwrap();

        // 等待 1 秒确保过期
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // 清理过期记录
        let cleaned = cache.cleanup_expired().await.unwrap();
        assert!(cleaned > 0);

        // 过期记录应该不存在
        let result = cache.check_or_record("expiring", 60).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cleanup_expired_keeps_valid_records() {
        let cache = IdempotencyCache::new();

        // 存储一个有效记录（TTL = 3600 秒）
        let response = serde_json::json!({ "valid": true });
        cache
            .store_response("valid-key", &response, 3600)
            .await
            .unwrap();

        // 清理过期记录
        let cleaned = cache.cleanup_expired().await.unwrap();
        assert_eq!(cleaned, 0);

        // 有效记录应该仍然存在
        let result = cache.check_or_record("valid-key", 60).await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_multiple_operations_on_same_key() {
        let cache = IdempotencyCache::new();
        let key = "multi-op-key";

        // 连续多次检查
        for _ in 0..3 {
            let check = cache.check_or_record(key, 60).await.unwrap();
            assert!(check.is_none());
        }

        // 存储响应
        let response = serde_json::json!({ "processed": true });
        cache.store_response(key, &response, 60).await.unwrap();

        // 之后检查应该返回响应
        let final_check = cache.check_or_record(key, 60).await.unwrap();
        assert!(final_check.is_some());
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
