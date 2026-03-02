//! # 数据库迁移
//!
//! 管理数据库 schema 版本和迁移。

use uhorse_core::Result;
use tracing::info;

/// 运行所有待处理的迁移
pub async fn run_migrations(_db_path: &str) -> Result<()> {
    info!("Running database migrations...");
    // TODO: 实现迁移系统
    Ok(())
}

/// 获取当前数据库版本
pub fn get_current_version() -> &'static str {
    "0.1.0"
}
