//! # Tracing 初始化
//!
//! 配置分布式追踪和结构化日志。

use super::telemetry::{init_observability, OtelConfig};

fn init_with_config(config: OtelConfig) -> anyhow::Result<()> {
    init_observability(config)
        .map_err(|e| anyhow::anyhow!("Failed to initialize observability: {:?}", e))?;
    Ok(())
}

/// 初始化 tracing（简化接口）
pub fn init_tracing(service_name: &str) -> anyhow::Result<()> {
    init_with_config(OtelConfig::new(service_name.to_string()))
}

/// 初始化基于 env filter 的控制台 tracing。
pub fn init_console_tracing(service_name: &str, env_filter: &str) -> anyhow::Result<()> {
    init_with_config(
        OtelConfig::new(service_name.to_string())
            .with_console_export(true)
            .with_env_filter(env_filter.to_string()),
    )
}

/// 初始化完整可观测性
pub fn init_full_observability(config: OtelConfig) -> anyhow::Result<()> {
    init_with_config(config)
}

/// 初始化开发环境可观测性（控制台输出）
pub fn init_dev_observability(service_name: &str) -> anyhow::Result<()> {
    init_console_tracing(service_name, "debug")
}
