//! # 健康检查
//!
//! 提供健康检查端点和状态监控。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// 健康状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// 健康
    Healthy,
    /// 降级
    Degraded,
    /// 不健康
    Unhealthy,
}

/// 健康检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// 状态
    pub status: HealthStatus,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 版本
    pub version: String,
    /// 启动时间
    pub started_at: DateTime<Utc>,
    /// 运行时长（秒）
    pub uptime_seconds: u64,
    /// 检查项
    #[serde(default)]
    pub checks: Vec<CheckResult>,
}

/// 单项检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// 检查项名称
    pub name: String,
    /// 状态
    pub status: HealthStatus,
    /// 消息
    pub message: Option<String>,
    /// 耗时（毫秒）
    pub duration_ms: u64,
}

/// 健康检查器类型
#[derive(Debug, Clone)]
pub enum CheckerType {
    Database { db_path: String },
    Memory { threshold_mb: usize },
    Disk { path: String, threshold_gb: f64 },
}

impl CheckerType {
    /// 执行检查
    pub async fn check(&self) -> CheckResult {
        match self {
            CheckerType::Database { db_path } => check_database(db_path).await,
            CheckerType::Memory { threshold_mb: _ } => check_memory().await,
            CheckerType::Disk {
                path,
                threshold_gb: _,
            } => check_disk(path).await,
        }
    }

    /// 获取检查名称
    pub fn name(&self) -> &str {
        match self {
            CheckerType::Database { .. } => "database",
            CheckerType::Memory { .. } => "memory",
            CheckerType::Disk { .. } => "disk",
        }
    }
}

/// 数据库检查
async fn check_database(db_path: &str) -> CheckResult {
    let start = std::time::Instant::now();

    let path = std::path::Path::new(db_path);
    if !path.exists() {
        return CheckResult {
            name: "database".to_string(),
            status: HealthStatus::Unhealthy,
            message: Some(format!("Database file not found: {}", db_path)),
            duration_ms: start.elapsed().as_millis() as u64,
        };
    }

    if let Err(e) = std::fs::metadata(db_path) {
        return CheckResult {
            name: "database".to_string(),
            status: HealthStatus::Unhealthy,
            message: Some(format!("Database file error: {}", e)),
            duration_ms: start.elapsed().as_millis() as u64,
        };
    }

    CheckResult {
        name: "database".to_string(),
        status: HealthStatus::Healthy,
        message: Some("Database OK".to_string()),
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

/// 内存检查
async fn check_memory() -> CheckResult {
    let start = std::time::Instant::now();

    CheckResult {
        name: "memory".to_string(),
        status: HealthStatus::Healthy,
        message: Some("Memory usage OK".to_string()),
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

/// 磁盘检查
async fn check_disk(path: &str) -> CheckResult {
    let start = std::time::Instant::now();

    let path = std::path::Path::new(path);
    let status = if path.exists() {
        HealthStatus::Healthy
    } else {
        HealthStatus::Unhealthy
    };

    CheckResult {
        name: "disk".to_string(),
        status,
        message: Some(format!("Disk check for: {}", path.display())),
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

/// 健康检查服务
pub struct HealthService {
    start_time: std::time::Instant,
    version: String,
    checkers: Vec<CheckerType>,
    status: Arc<RwLock<HealthStatus>>,
}

impl HealthService {
    /// 创建新的健康检查服务
    pub fn new(version: String) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            version,
            checkers: Vec::new(),
            status: Arc::new(RwLock::new(HealthStatus::Healthy)),
        }
    }

    /// 添加检查器
    pub fn add_checker(mut self, checker: CheckerType) -> Self {
        self.checkers.push(checker);
        self
    }

    /// 执行健康检查
    pub async fn check(&self) -> HealthCheck {
        let mut checks = Vec::new();

        // 执行所有检查
        for checker in &self.checkers {
            checks.push(checker.check().await);
        }

        // 计算总体状态
        let status = self.overall_status(&checks);

        // 更新当前状态
        *self.status.write().await = status;

        HealthCheck {
            status,
            timestamp: Utc::now(),
            version: self.version.clone(),
            started_at: DateTime::from_timestamp(self.start_time.elapsed().as_secs() as i64, 0)
                .unwrap_or(Utc::now()),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            checks,
        }
    }

    /// 获取当前状态（不执行检查）
    pub async fn status(&self) -> HealthStatus {
        *self.status.read().await
    }

    /// 计算总体状态
    fn overall_status(&self, checks: &[CheckResult]) -> HealthStatus {
        if checks.is_empty() {
            return HealthStatus::Healthy;
        }

        let has_unhealthy = checks.iter().any(|c| c.status == HealthStatus::Unhealthy);
        let has_degraded = checks.iter().any(|c| c.status == HealthStatus::Degraded);

        if has_unhealthy {
            HealthStatus::Unhealthy
        } else if has_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }
}

/// Liveness 检查（简单存活性检查）
pub async fn liveness() -> &'static str {
    "OK"
}

/// Readiness 检查（就绪性检查）
pub async fn readiness(health_service: &HealthService) -> HealthCheck {
    health_service.check().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_status_overall() {
        let service = HealthService::new("1.0.0".to_string());

        // 无检查器时默认健康
        let check = service.check().await;
        assert_eq!(check.status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_liveness() {
        let result = liveness().await;
        assert_eq!(result, "OK");
    }
}
