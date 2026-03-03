//! # 日志轮转
//!
//! 提供日志文件轮转和管理功能。

use chrono::{DateTime, Duration, Utc};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{info, warn};

/// 轮转策略
#[derive(Debug, Clone, Copy)]
pub enum RotationStrategy {
    Size {
        max_size_mb: u64,
        max_files: usize,
    },
    Time {
        interval_hours: u64,
        max_files: usize,
    },
    Daily {
        max_files: usize,
    },
}

impl RotationStrategy {
    pub fn max_files(&self) -> usize {
        match self {
            RotationStrategy::Size { max_files, .. } => *max_files,
            RotationStrategy::Time { max_files, .. } => *max_files,
            RotationStrategy::Daily { max_files } => *max_files,
        }
    }
}

/// 日志轮转器
pub struct LogRotator {
    log_dir: PathBuf,
    file_prefix: String,
    strategy: RotationStrategy,
    current_size: u64,
}

impl LogRotator {
    pub fn new(log_dir: PathBuf, file_prefix: String, strategy: RotationStrategy) -> Self {
        Self {
            log_dir,
            file_prefix,
            strategy,
            current_size: 0,
        }
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.log_dir).await?;
        Ok(())
    }

    pub fn should_rotate(&self) -> bool {
        match self.strategy {
            RotationStrategy::Size { max_size_mb, .. } => {
                let max_bytes = max_size_mb * 1024 * 1024;
                self.current_size >= max_bytes
            }
            _ => false,
        }
    }

    pub async fn rotate(&mut self) -> anyhow::Result<PathBuf> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let backup_name = format!("{}.{}.log.bak", self.file_prefix, timestamp);
        let backup_path = self.log_dir.join(&backup_name);

        let current_path = self.log_dir.join(format!("{}.log", self.file_prefix));
        if current_path.exists() {
            fs::rename(&current_path, &backup_path).await?;
        }

        fs::File::create(&current_path).await?;
        self.current_size = 0;

        Ok(current_path)
    }
}

/// 日志归档器
pub struct LogArchiver {
    archive_dir: PathBuf,
}

impl LogArchiver {
    pub fn new(archive_dir: PathBuf) -> Self {
        Self { archive_dir }
    }

    pub async fn cleanup_expired_archives(&self, retain_days: i64) -> anyhow::Result<()> {
        use tokio::task::spawn_blocking;

        let archive_dir = self.archive_dir.clone();

        spawn_blocking(move || {
            if !archive_dir.exists() {
                return Ok::<(), anyhow::Error>(());
            }

            let entries = std::fs::read_dir(&archive_dir)?;

            for entry in entries {
                let entry = entry?;
                let path = entry.path();

                if let Ok(metadata) = std::fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        let system_time: std::time::SystemTime = modified;
                        let datetime: DateTime<Utc> = system_time.into();
                        let duration = Utc::now() - datetime;
                        let days_old = duration.num_seconds() / 86400;

                        if days_old > retain_days {
                            info!("Removing expired archive: {:?}", path);
                            std::fs::remove_file(&path)?;
                        }
                    }
                }
            }

            Ok::<(), anyhow::Error>(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("Cleanup task failed: {}", e))??;

        Ok(())
    }
}
