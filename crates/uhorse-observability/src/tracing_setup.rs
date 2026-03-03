//! # Tracing 初始化
//!
//! 配置分布式追踪和结构化日志。

use super::telemetry::{init_observability, OtelConfig};

/// 初始化 tracing（简化接口）
pub fn init_tracing(service_name: &str) -> anyhow::Result<()> {
    let config = OtelConfig::new(service_name.to_string());
    init_observability(config)
        .map_err(|e| anyhow::anyhow!("Failed to initialize observability: {:?}", e))?;
    Ok(())
}

/// 初始化完整可观测性
pub fn init_full_observability(config: OtelConfig) -> anyhow::Result<()> {
    init_observability(config)
        .map_err(|e| anyhow::anyhow!("Failed to initialize observability: {:?}", e))?;
    Ok(())
}

/// 初始化开发环境可观测性（控制台输出）
pub fn init_dev_observability(service_name: &str) -> anyhow::Result<()> {
    let config = OtelConfig::new(service_name.to_string())
        .with_console_export(true)
        .with_env_filter("debug".to_string());
    init_observability(config)
        .map_err(|e| anyhow::anyhow!("Failed to initialize observability: {:?}", e))?;
    Ok(())
}
