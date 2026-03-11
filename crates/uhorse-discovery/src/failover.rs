//! Failover mechanism
//!
//! 故障转移机制，支持自动故障检测和转移

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use super::types::ServiceInstance;

/// 故障转移策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailoverStrategy {
    /// 自动故障转移
    Automatic,
    /// 手动故障转移
    Manual,
    /// 优先级故障转移
    Priority,
}

/// 故障转移状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailoverStatus {
    /// 正常运行
    Healthy,
    /// 降级运行
    Degraded,
    /// 故障转移中
    FailingOver,
    /// 已故障转移
    FailedOver,
    /// 恢复中
    Recovering,
}

/// 故障记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecord {
    /// 记录 ID
    pub id: String,
    /// 服务实例 ID
    pub instance_id: String,
    /// 服务名称
    pub service_name: String,
    /// 故障类型
    pub failure_type: FailureType,
    /// 发生时间
    pub occurred_at: DateTime<Utc>,
    /// 恢复时间
    pub recovered_at: Option<DateTime<Utc>>,
    /// 故障描述
    pub description: String,
}

impl FailureRecord {
    /// 创建新的故障记录
    pub fn new(
        instance_id: impl Into<String>,
        service_name: impl Into<String>,
        failure_type: FailureType,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            instance_id: instance_id.into(),
            service_name: service_name.into(),
            failure_type,
            occurred_at: Utc::now(),
            recovered_at: None,
            description: description.into(),
        }
    }
}

/// 故障类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureType {
    /// 健康检查失败
    HealthCheckFailed,
    /// 连接超时
    ConnectionTimeout,
    /// 服务不可用
    ServiceUnavailable,
    /// 资源耗尽
    ResourceExhausted,
    /// 网络分区
    NetworkPartition,
    /// 未知错误
    Unknown,
}

/// 故障转移配置
#[derive(Debug, Clone)]
pub struct FailoverConfig {
    /// 故障转移策略
    pub strategy: FailoverStrategy,
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试间隔 (毫秒)
    pub retry_interval_ms: u64,
    /// 故障检测超时 (秒)
    pub detection_timeout_secs: u64,
    /// 故障恢复等待时间 (秒)
    pub recovery_wait_secs: u64,
    /// 是否启用自动恢复
    pub auto_recovery: bool,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            strategy: FailoverStrategy::Automatic,
            max_retries: 3,
            retry_interval_ms: 1000,
            detection_timeout_secs: 30,
            recovery_wait_secs: 60,
            auto_recovery: true,
        }
    }
}

/// 故障转移记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverRecord {
    /// 记录 ID
    pub id: String,
    /// 服务名称
    pub service_name: String,
    /// 源实例 ID
    pub source_instance_id: String,
    /// 目标实例 ID
    pub target_instance_id: Option<String>,
    /// 状态
    pub status: FailoverStatus,
    /// 开始时间
    pub started_at: DateTime<Utc>,
    /// 完成时间
    pub completed_at: Option<DateTime<Utc>>,
    /// 失败原因
    pub failure_reason: Option<String>,
    /// 重试次数
    pub retry_count: u32,
}

impl FailoverRecord {
    /// 创建新的故障转移记录
    pub fn new(
        service_name: impl Into<String>,
        source_instance_id: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            service_name: service_name.into(),
            source_instance_id: source_instance_id.into(),
            target_instance_id: None,
            status: FailoverStatus::FailingOver,
            started_at: Utc::now(),
            completed_at: None,
            failure_reason: None,
            retry_count: 0,
        }
    }

    /// 获取故障转移耗时 (秒)
    pub fn duration_secs(&self) -> Option<u64> {
        self.completed_at
            .map(|completed| (completed - self.started_at).num_seconds() as u64)
    }
}

/// 故障转移管理器
pub struct FailoverManager {
    /// 配置
    config: FailoverConfig,
    /// 故障转移记录
    records: Arc<RwLock<Vec<FailoverRecord>>>,
    /// 故障记录
    failures: Arc<RwLock<Vec<FailureRecord>>>,
    /// 服务状态
    service_status: Arc<RwLock<HashMap<String, FailoverStatus>>>,
}

impl FailoverManager {
    /// 创建新的故障转移管理器
    pub fn new(config: FailoverConfig) -> Self {
        Self {
            config,
            records: Arc::new(RwLock::new(Vec::new())),
            failures: Arc::new(RwLock::new(Vec::new())),
            service_status: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 检测故障
    pub async fn detect_failure(
        &self,
        instance: &ServiceInstance,
        failure_type: FailureType,
        description: impl Into<String>,
    ) -> super::Result<FailureRecord> {
        let record = FailureRecord::new(
            &instance.id,
            instance.service_name(),
            failure_type,
            description,
        );

        // 更新服务状态
        let mut status = self.service_status.write().await;
        status.insert(instance.id.clone(), FailoverStatus::Degraded);

        // 保存故障记录
        let mut failures = self.failures.write().await;
        failures.push(record.clone());

        info!(
            "Detected failure for instance {}: {:?}",
            instance.id, failure_type
        );

        Ok(record)
    }

    /// 执行故障转移
    pub async fn execute_failover(
        &self,
        service_name: &str,
        failed_instance_id: &str,
        healthy_instances: &[ServiceInstance],
    ) -> super::Result<FailoverRecord> {
        // 检查是否有可用的健康实例
        if healthy_instances.is_empty() {
            return Err(super::Error::NoHealthyInstances(service_name.to_string()));
        }

        let mut record = FailoverRecord::new(service_name, failed_instance_id);

        // 根据策略选择目标实例
        let target = match self.config.strategy {
            FailoverStrategy::Automatic => healthy_instances.first(),
            FailoverStrategy::Manual => {
                // 手动模式需要外部指定，这里默认选择第一个
                healthy_instances.first()
            }
            FailoverStrategy::Priority => {
                // 按优先级排序 (这里简化为按 ID 排序)
                healthy_instances.iter().min_by_key(|i| &i.id)
            }
        };

        if let Some(target_instance) = target {
            record.target_instance_id = Some(target_instance.id.clone());
            record.status = FailoverStatus::FailedOver;
            record.completed_at = Some(Utc::now());

            // 更新服务状态
            let mut status = self.service_status.write().await;
            status.insert(failed_instance_id.to_string(), FailoverStatus::FailedOver);
            status.insert(target_instance.id.clone(), FailoverStatus::Healthy);

            info!(
                "Failover completed: {} -> {} for service {}",
                failed_instance_id, target_instance.id, service_name
            );
        } else {
            record.status = FailoverStatus::Degraded;
            record.failure_reason = Some("No healthy instances available".to_string());

            warn!(
                "Failover failed: no healthy instances for service {}",
                service_name
            );
        }

        // 保存记录
        let mut records = self.records.write().await;
        records.push(record.clone());

        Ok(record)
    }

    /// 恢复服务
    pub async fn recover_service(
        &self,
        instance_id: &str,
        service_name: &str,
    ) -> super::Result<bool> {
        let mut status = self.service_status.write().await;

        if let Some(current_status) = status.get_mut(instance_id) {
            *current_status = FailoverStatus::Recovering;

            // 模拟恢复过程
            // 实际应用中应该执行健康检查
            *current_status = FailoverStatus::Healthy;

            // 更新故障记录
            let mut failures = self.failures.write().await;
            for failure in failures.iter_mut() {
                if failure.instance_id == instance_id && failure.recovered_at.is_none() {
                    failure.recovered_at = Some(Utc::now());
                }
            }

            info!("Service recovered: {} ({})", instance_id, service_name);

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 获取服务状态
    pub async fn get_service_status(&self, instance_id: &str) -> Option<FailoverStatus> {
        let status = self.service_status.read().await;
        status.get(instance_id).copied()
    }

    /// 获取故障转移记录
    pub async fn get_record(&self, record_id: &str) -> Option<FailoverRecord> {
        let records = self.records.read().await;
        records.iter().find(|r| r.id == record_id).cloned()
    }

    /// 列出所有故障转移记录
    pub async fn list_records(&self) -> Vec<FailoverRecord> {
        self.records.read().await.clone()
    }

    /// 列出所有故障记录
    pub async fn list_failures(&self) -> Vec<FailureRecord> {
        self.failures.read().await.clone()
    }

    /// 获取故障转移统计
    pub async fn stats(&self) -> FailoverStats {
        let records = self.records.read().await;
        let failures = self.failures.read().await;
        let status = self.service_status.read().await;

        let mut stats = FailoverStats::default();
        stats.total_failovers = records.len() as u64;
        stats.total_failures = failures.len() as u64;

        for record in records.iter() {
            match record.status {
                FailoverStatus::FailedOver => stats.successful_failovers += 1,
                FailoverStatus::Degraded => stats.degraded_services += 1,
                FailoverStatus::FailingOver => stats.in_progress_failovers += 1,
                FailoverStatus::Recovering => stats.recovering_services += 1,
                FailoverStatus::Healthy => {}
            }
        }

        for failure in failures.iter() {
            if failure.recovered_at.is_none() {
                stats.active_failures += 1;
            }
        }

        stats.healthy_services = status
            .values()
            .filter(|&&s| s == FailoverStatus::Healthy)
            .count() as u64;

        stats
    }
}

impl Default for FailoverManager {
    fn default() -> Self {
        Self::new(FailoverConfig::default())
    }
}

/// 故障转移统计
#[derive(Debug, Clone, Default)]
pub struct FailoverStats {
    /// 总故障转移次数
    pub total_failovers: u64,
    /// 成功故障转移次数
    pub successful_failovers: u64,
    /// 进行中的故障转移
    pub in_progress_failovers: u64,
    /// 总故障次数
    pub total_failures: u64,
    /// 活跃故障数
    pub active_failures: u64,
    /// 降级服务数
    pub degraded_services: u64,
    /// 恢复中服务数
    pub recovering_services: u64,
    /// 健康服务数
    pub healthy_services: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failure_record_creation() {
        let record = FailureRecord::new(
            "instance-1",
            "test-service",
            FailureType::HealthCheckFailed,
            "Health check failed",
        );

        assert_eq!(record.instance_id, "instance-1");
        assert_eq!(record.failure_type, FailureType::HealthCheckFailed);
        assert!(record.recovered_at.is_none());
    }

    #[test]
    fn test_failover_record_creation() {
        let record = FailoverRecord::new("test-service", "instance-1");

        assert_eq!(record.service_name, "test-service");
        assert_eq!(record.source_instance_id, "instance-1");
        assert!(record.target_instance_id.is_none());
        assert_eq!(record.status, FailoverStatus::FailingOver);
    }

    #[tokio::test]
    async fn test_detect_failure() {
        let manager = FailoverManager::default();
        let instance = ServiceInstance::new(
            "instance-1",
            "test-service",
            "127.0.0.1",
            8080,
        );

        let record = manager
            .detect_failure(&instance, FailureType::ConnectionTimeout, "Connection timed out")
            .await
            .unwrap();

        assert_eq!(record.instance_id, "instance-1");
        assert_eq!(record.failure_type, FailureType::ConnectionTimeout);

        let status = manager.get_service_status("instance-1").await;
        assert_eq!(status, Some(FailoverStatus::Degraded));
    }

    #[tokio::test]
    async fn test_failover_stats() {
        let manager = FailoverManager::default();
        let stats = manager.stats().await;

        assert_eq!(stats.total_failovers, 0);
        assert_eq!(stats.total_failures, 0);
    }
}
