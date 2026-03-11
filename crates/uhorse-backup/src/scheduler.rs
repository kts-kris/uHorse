//! Backup scheduler
//!
//! 自动备份调度，支持完整备份和增量备份

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// 备份类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupType {
    /// 完整备份
    Full,
    /// 增量备份
    Incremental,
}

/// 备份状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupStatus {
    /// 待执行
    Pending,
    /// 进行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed,
}

/// 备份记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    /// 备份 ID
    pub id: String,
    /// 备份类型
    pub backup_type: BackupType,
    /// 状态
    pub status: BackupStatus,
    /// 存储路径
    pub storage_path: PathBuf,
    /// 大小 (字节)
    pub size_bytes: u64,
    /// 校验和
    pub checksum: String,
    /// 基础备份 ID (增量备份时)
    pub base_backup_id: Option<String>,
    /// 加密密钥 ID
    pub encryption_key_id: Option<String>,
    /// 开始时间
    pub started_at: DateTime<Utc>,
    /// 完成时间
    pub completed_at: Option<DateTime<Utc>>,
    /// 错误信息
    pub error_message: Option<String>,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

impl BackupRecord {
    /// 创建新的备份记录
    pub fn new(backup_type: BackupType, storage_path: PathBuf) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            backup_type,
            status: BackupStatus::Pending,
            storage_path,
            size_bytes: 0,
            checksum: String::new(),
            base_backup_id: None,
            encryption_key_id: None,
            started_at: Utc::now(),
            completed_at: None,
            error_message: None,
            metadata: HashMap::new(),
        }
    }

    /// 设置基础备份 (用于增量备份)
    pub fn with_base(mut self, base_id: impl Into<String>) -> Self {
        self.base_backup_id = Some(base_id.into());
        self
    }

    /// 获取备份耗时 (秒)
    pub fn duration_secs(&self) -> Option<u64> {
        self.completed_at
            .map(|completed| (completed - self.started_at).num_seconds() as u64)
    }
}

/// 备份调度配置
#[derive(Debug, Clone)]
pub struct BackupScheduleConfig {
    /// 备份存储目录
    pub backup_dir: PathBuf,
    /// 完整备份间隔 (小时)
    pub full_backup_interval_hours: u32,
    /// 增量备份间隔 (小时)
    pub incremental_backup_interval_hours: u32,
    /// 保留备份数量
    pub retention_count: u32,
    /// 是否启用压缩
    pub compress: bool,
    /// 是否启用加密
    pub encrypt: bool,
}

impl Default for BackupScheduleConfig {
    fn default() -> Self {
        Self {
            backup_dir: PathBuf::from("backups"),
            full_backup_interval_hours: 24,
            incremental_backup_interval_hours: 4,
            retention_count: 30,
            compress: true,
            encrypt: true,
        }
    }
}

/// 备份调度器
pub struct BackupScheduler {
    /// 配置
    config: BackupScheduleConfig,
    /// 备份记录
    records: Arc<RwLock<Vec<BackupRecord>>>,
    /// 上次完整备份时间
    last_full_backup: Arc<RwLock<Option<DateTime<Utc>>>>,
    /// 上次增量备份时间
    last_incremental_backup: Arc<RwLock<Option<DateTime<Utc>>>>,
}

impl BackupScheduler {
    /// 创建新的备份调度器
    pub fn new(config: BackupScheduleConfig) -> Self {
        Self {
            config,
            records: Arc::new(RwLock::new(Vec::new())),
            last_full_backup: Arc::new(RwLock::new(None)),
            last_incremental_backup: Arc::new(RwLock::new(None)),
        }
    }

    /// 确保备份目录存在
    pub async fn ensure_backup_dir(&self) -> std::io::Result<()> {
        tokio::fs::create_dir_all(&self.config.backup_dir).await
    }

    /// 检查是否需要完整备份
    pub async fn needs_full_backup(&self) -> bool {
        let last = self.last_full_backup.read().await;
        match *last {
            None => true,
            Some(time) => {
                let elapsed = Utc::now() - time;
                elapsed.num_hours() >= self.config.full_backup_interval_hours as i64
            }
        }
    }

    /// 检查是否需要增量备份
    pub async fn needs_incremental_backup(&self) -> bool {
        // 如果需要完整备份，则不需要增量备份
        if self.needs_full_backup().await {
            return false;
        }

        let last = self.last_incremental_backup.read().await;
        match *last {
            None => true,
            Some(time) => {
                let elapsed = Utc::now() - time;
                elapsed.num_hours() >= self.config.incremental_backup_interval_hours as i64
            }
        }
    }

    /// 创建备份记录
    pub async fn create_backup_record(&self, backup_type: BackupType) -> BackupRecord {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = match backup_type {
            BackupType::Full => format!("full_{}.backup", timestamp),
            BackupType::Incremental => format!("incremental_{}.backup", timestamp),
        };

        let storage_path = self.config.backup_dir.join(&filename);
        let mut record = BackupRecord::new(backup_type, storage_path);

        // 如果是增量备份，找到最近的基础备份
        if backup_type == BackupType::Incremental {
            let records = self.records.read().await;
            if let Some(last_full) = records
                .iter()
                .filter(|r| r.backup_type == BackupType::Full && r.status == BackupStatus::Completed)
                .max_by_key(|r| r.started_at)
            {
                record = record.with_base(&last_full.id);
            }
        }

        record
    }

    /// 开始备份
    pub async fn start_backup(&self, record: &mut BackupRecord) {
        record.status = BackupStatus::Running;
        record.started_at = Utc::now();
    }

    /// 完成备份
    pub async fn complete_backup(&self, record: &mut BackupRecord, size_bytes: u64, checksum: String) {
        record.status = BackupStatus::Completed;
        record.completed_at = Some(Utc::now());
        record.size_bytes = size_bytes;
        record.checksum = checksum;

        // 更新最后备份时间
        match record.backup_type {
            BackupType::Full => {
                let mut last = self.last_full_backup.write().await;
                *last = Some(record.started_at);
            }
            BackupType::Incremental => {
                let mut last = self.last_incremental_backup.write().await;
                *last = Some(record.started_at);
            }
        }

        // 保存记录
        let mut records = self.records.write().await;
        records.push(record.clone());

        info!(
            "Backup completed: {} ({} bytes, type: {:?})",
            record.id, record.size_bytes, record.backup_type
        );
    }

    /// 备份失败
    pub async fn fail_backup(&self, record: &mut BackupRecord, error: String) {
        record.status = BackupStatus::Failed;
        record.completed_at = Some(Utc::now());
        record.error_message = Some(error);

        let mut records = self.records.write().await;
        records.push(record.clone());
    }

    /// 获取备份记录
    pub async fn get_record(&self, backup_id: &str) -> Option<BackupRecord> {
        let records = self.records.read().await;
        records.iter().find(|r| r.id == backup_id).cloned()
    }

    /// 列出所有备份
    pub async fn list_backups(&self) -> Vec<BackupRecord> {
        let records = self.records.read().await;
        records.clone()
    }

    /// 列出指定类型的备份
    pub async fn list_backups_by_type(&self, backup_type: BackupType) -> Vec<BackupRecord> {
        let records = self.records.read().await;
        records
            .iter()
            .filter(|r| r.backup_type == backup_type)
            .cloned()
            .collect()
    }

    /// 清理过期备份
    pub async fn cleanup_old_backups(&self) -> super::Result<u64> {
        let mut records = self.records.write().await;

        // 按时间排序
        records.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        // 保留最近的 N 个备份
        let to_remove: Vec<_> = records
            .iter()
            .skip(self.config.retention_count as usize)
            .map(|r| r.id.clone())
            .collect();

        let mut cleaned = 0u64;
        for id in to_remove {
            if let Some(pos) = records.iter().position(|r| r.id == id) {
                let record = records.remove(pos);
                // 删除备份文件
                if let Err(e) = tokio::fs::remove_file(&record.storage_path).await {
                    tracing::warn!("Failed to delete backup file: {}", e);
                }
                cleaned += 1;
            }
        }

        if cleaned > 0 {
            info!("Cleaned up {} old backups", cleaned);
        }

        Ok(cleaned)
    }

    /// 获取备份统计
    pub async fn stats(&self) -> BackupStats {
        let records = self.records.read().await;

        let mut stats = BackupStats::default();
        stats.total_backups = records.len() as u64;

        for record in records.iter() {
            match record.status {
                BackupStatus::Completed => stats.completed_count += 1,
                BackupStatus::Pending => stats.pending_count += 1,
                BackupStatus::Running => stats.running_count += 1,
                BackupStatus::Failed => stats.failed_count += 1,
            }

            if record.status == BackupStatus::Completed {
                stats.total_size_bytes += record.size_bytes;

                match record.backup_type {
                    BackupType::Full => stats.full_backup_count += 1,
                    BackupType::Incremental => stats.incremental_backup_count += 1,
                }
            }
        }

        stats
    }
}

impl Default for BackupScheduler {
    fn default() -> Self {
        Self::new(BackupScheduleConfig::default())
    }
}

/// 备份统计
#[derive(Debug, Clone, Default)]
pub struct BackupStats {
    /// 总备份数
    pub total_backups: u64,
    /// 已完成数
    pub completed_count: u64,
    /// 进行中数
    pub running_count: u64,
    /// 待执行数
    pub pending_count: u64,
    /// 失败数
    pub failed_count: u64,
    /// 完整备份数
    pub full_backup_count: u64,
    /// 增量备份数
    pub incremental_backup_count: u64,
    /// 总大小 (字节)
    pub total_size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_record_creation() {
        let record = BackupRecord::new(BackupType::Full, PathBuf::from("/tmp/backup"));

        assert_eq!(record.status, BackupStatus::Pending);
        assert_eq!(record.backup_type, BackupType::Full);
        assert!(record.completed_at.is_none());
    }

    #[test]
    fn test_backup_record_with_base() {
        let record = BackupRecord::new(BackupType::Incremental, PathBuf::from("/tmp/backup"))
            .with_base("base-backup-id");

        assert!(record.base_backup_id.is_some());
        assert_eq!(record.base_backup_id.unwrap(), "base-backup-id");
    }

    #[tokio::test]
    async fn test_scheduler_needs_backup() {
        let scheduler = BackupScheduler::default();

        // 新创建的调度器应该需要备份
        assert!(scheduler.needs_full_backup().await);
    }

    #[tokio::test]
    async fn test_scheduler_stats() {
        let scheduler = BackupScheduler::default();
        let stats = scheduler.stats().await;

        assert_eq!(stats.total_backups, 0);
    }
}
