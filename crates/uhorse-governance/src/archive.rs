//! Data archiving
//!
//! 数据归档，将冷数据移动到归档存储

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use super::classification::{DataType, SensitivityLevel};

/// 归档状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchiveStatus {
    /// 待归档
    Pending,
    /// 归档中
    Archiving,
    /// 已归档
    Archived,
    /// 归档失败
    Failed,
    /// 已恢复
    Restored,
}

/// 归档记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveRecord {
    /// 归档 ID
    pub id: String,
    /// 原始数据 ID
    pub source_id: String,
    /// 数据类型
    pub data_type: DataType,
    /// 敏感度级别
    pub sensitivity: SensitivityLevel,
    /// 归档路径
    pub archive_path: PathBuf,
    /// 原始大小 (字节)
    pub original_size: u64,
    /// 压缩大小 (字节)
    pub compressed_size: u64,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 归档时间
    pub archived_at: Option<DateTime<Utc>>,
    /// 状态
    pub status: ArchiveStatus,
    /// 校验和
    pub checksum: String,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

impl ArchiveRecord {
    /// 创建新的归档记录
    pub fn new(
        source_id: impl Into<String>,
        data_type: DataType,
        sensitivity: SensitivityLevel,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source_id: source_id.into(),
            data_type,
            sensitivity,
            archive_path: PathBuf::new(),
            original_size: 0,
            compressed_size: 0,
            created_at: Utc::now(),
            archived_at: None,
            status: ArchiveStatus::Pending,
            checksum: String::new(),
            metadata: HashMap::new(),
        }
    }

    /// 获取压缩率
    pub fn compression_ratio(&self) -> f32 {
        if self.original_size == 0 {
            return 0.0;
        }
        1.0 - (self.compressed_size as f32 / self.original_size as f32)
    }
}

/// 归档配置
#[derive(Debug, Clone)]
pub struct ArchiveConfig {
    /// 归档目录
    pub archive_dir: PathBuf,
    /// 最大归档大小 (字节)
    pub max_archive_size: u64,
    /// 保留天数
    pub retention_days: u32,
    /// 是否压缩
    pub compress: bool,
    /// 是否加密
    pub encrypt: bool,
}

impl Default for ArchiveConfig {
    fn default() -> Self {
        Self {
            archive_dir: PathBuf::from("archives"),
            max_archive_size: 1024 * 1024 * 1024, // 1GB
            retention_days: 365,
            compress: true,
            encrypt: false,
        }
    }
}

/// 归档管理器
pub struct ArchiveManager {
    /// 配置
    config: ArchiveConfig,
    /// 归档记录
    records: Arc<RwLock<HashMap<String, ArchiveRecord>>>,
}

impl ArchiveManager {
    /// 创建新的归档管理器
    pub fn new(config: ArchiveConfig) -> Self {
        Self {
            config,
            records: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 确保归档目录存在
    pub async fn ensure_archive_dir(&self) -> std::io::Result<()> {
        tokio::fs::create_dir_all(&self.config.archive_dir).await
    }

    /// 创建归档
    pub async fn create_archive(
        &self,
        source_id: &str,
        data_type: DataType,
        sensitivity: SensitivityLevel,
        data: &[u8],
    ) -> super::Result<String> {
        // 创建归档记录
        let mut record = ArchiveRecord::new(source_id, data_type, sensitivity);
        record.original_size = data.len() as u64;
        record.status = ArchiveStatus::Archiving;

        // 生成归档路径
        let date_path = Utc::now().format("%Y/%m/%d").to_string();
        record.archive_path = self
            .config
            .archive_dir
            .join(&date_path)
            .join(format!("{}.archive", record.id));

        // 确保目录存在
        if let Some(parent) = record.archive_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| super::GovernanceError::ArchiveError(e.to_string()))?;
        }

        // 写入归档数据 (简化实现，实际应包含压缩和加密)
        let archive_data = if self.config.compress {
            // 这里应该使用实际的压缩库
            data.to_vec()
        } else {
            data.to_vec()
        };

        record.compressed_size = archive_data.len() as u64;

        // 计算校验和
        record.checksum = format!("{:x}", Self::compute_checksum(&archive_data));

        // 写入文件
        tokio::fs::write(&record.archive_path, &archive_data)
            .await
            .map_err(|e| super::GovernanceError::ArchiveError(e.to_string()))?;

        // 更新记录
        record.status = ArchiveStatus::Archived;
        record.archived_at = Some(Utc::now());

        let archive_id = record.id.clone();

        // 存储记录
        let mut records = self.records.write().await;
        records.insert(archive_id.clone(), record.clone());

        info!(
            "Created archive {} for source {} ({} bytes -> {} bytes, {:.1}% compression)",
            archive_id,
            source_id,
            record.original_size,
            record.compressed_size,
            record.compression_ratio() * 100.0
        );

        Ok(archive_id)
    }

    /// 恢复归档
    pub async fn restore_archive(&self, archive_id: &str) -> super::Result<Vec<u8>> {
        let records = self.records.read().await;

        let record = records
            .get(archive_id)
            .ok_or_else(|| super::GovernanceError::NotFound(archive_id.to_string()))?;

        if record.status != ArchiveStatus::Archived {
            return Err(super::GovernanceError::ArchiveError(format!(
                "Archive not ready for restore: {:?}",
                record.status
            )));
        }

        // 读取归档数据
        let data = tokio::fs::read(&record.archive_path)
            .await
            .map_err(|e| super::GovernanceError::ArchiveError(e.to_string()))?;

        // 验证校验和
        let computed = format!("{:x}", Self::compute_checksum(&data));
        if computed != record.checksum {
            error!(
                "Archive checksum mismatch for {}: expected {}, got {}",
                archive_id, record.checksum, computed
            );
            return Err(super::GovernanceError::ArchiveError(
                "Checksum verification failed".to_string(),
            ));
        }

        info!("Restored archive {} ({} bytes)", archive_id, data.len());
        Ok(data)
    }

    /// 删除归档
    pub async fn delete_archive(&self, archive_id: &str) -> super::Result<bool> {
        let mut records = self.records.write().await;

        if let Some(record) = records.remove(archive_id) {
            // 删除归档文件
            if let Err(e) = tokio::fs::remove_file(&record.archive_path).await {
                warn!("Failed to delete archive file: {}", e);
            }

            info!("Deleted archive {}", archive_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 获取归档记录
    pub async fn get_record(&self, archive_id: &str) -> Option<ArchiveRecord> {
        let records = self.records.read().await;
        records.get(archive_id).cloned()
    }

    /// 列出所有归档
    pub async fn list_archives(&self) -> Vec<ArchiveRecord> {
        let records = self.records.read().await;
        records.values().cloned().collect()
    }

    /// 获取归档统计
    pub async fn stats(&self) -> ArchiveStats {
        let records = self.records.read().await;

        let mut stats = ArchiveStats::default();
        stats.total_archives = records.len() as u64;

        for record in records.values() {
            stats.total_original_size += record.original_size;
            stats.total_compressed_size += record.compressed_size;

            match record.status {
                ArchiveStatus::Archived => stats.archived_count += 1,
                ArchiveStatus::Pending => stats.pending_count += 1,
                ArchiveStatus::Failed => stats.failed_count += 1,
                _ => {}
            }

            let category = format!("{:?}", record.data_type);
            *stats.by_data_type.entry(category).or_insert(0) += 1;
        }

        stats
    }

    /// 清理过期归档
    pub async fn cleanup_expired(&self) -> super::Result<u64> {
        let threshold = Utc::now() - chrono::Duration::days(self.config.retention_days as i64);
        let mut records = self.records.write().await;
        let mut cleaned = 0u64;

        let to_delete: Vec<String> = records
            .iter()
            .filter(|(_, r)| r.archived_at.map(|t| t < threshold).unwrap_or(false))
            .map(|(id, _)| id.clone())
            .collect();

        for id in to_delete {
            if let Some(record) = records.remove(&id) {
                if let Err(e) = tokio::fs::remove_file(&record.archive_path).await {
                    warn!("Failed to delete expired archive file: {}", e);
                }
                cleaned += 1;
            }
        }

        if cleaned > 0 {
            info!("Cleaned up {} expired archives", cleaned);
        }

        Ok(cleaned)
    }

    /// 计算校验和
    fn compute_checksum(data: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }
}

impl Default for ArchiveManager {
    fn default() -> Self {
        Self::new(ArchiveConfig::default())
    }
}

/// 归档统计
#[derive(Debug, Clone, Default)]
pub struct ArchiveStats {
    /// 总归档数
    pub total_archives: u64,
    /// 已归档数
    pub archived_count: u64,
    /// 待归档数
    pub pending_count: u64,
    /// 失败数
    pub failed_count: u64,
    /// 原始总大小
    pub total_original_size: u64,
    /// 压缩总大小
    pub total_compressed_size: u64,
    /// 按数据类型统计
    pub by_data_type: HashMap<String, u64>,
}

impl ArchiveStats {
    /// 获取总压缩率
    pub fn total_compression_ratio(&self) -> f32 {
        if self.total_original_size == 0 {
            return 0.0;
        }
        1.0 - (self.total_compressed_size as f32 / self.total_original_size as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_record_creation() {
        let record = ArchiveRecord::new(
            "source-1",
            DataType::TechnicalLogs,
            SensitivityLevel::Internal,
        );

        assert_eq!(record.status, ArchiveStatus::Pending);
        assert!(record.archived_at.is_none());
    }

    #[test]
    fn test_compression_ratio() {
        let mut record = ArchiveRecord::new(
            "source-1",
            DataType::TechnicalLogs,
            SensitivityLevel::Internal,
        );

        record.original_size = 1000;
        record.compressed_size = 500;

        assert!((record.compression_ratio() - 0.5).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_archive_manager_stats() {
        let manager = ArchiveManager::default();
        let stats = manager.stats().await;

        assert_eq!(stats.total_archives, 0);
    }
}
