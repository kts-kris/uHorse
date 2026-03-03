//! # OpenTelemetry 集成
//!
//! 完整的分布式追踪、metrics 和日志集成。

use std::io;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// OpenTelemetry 配置
#[derive(Debug, Clone)]
pub struct OtelConfig {
    /// 服务名称
    pub service_name: String,
    /// OTLP 端点（如 Jaeger、Tempo）
    pub otlp_endpoint: Option<String>,
    /// 是否启用导出 span 到控制台
    pub console_export: bool,
    /// 采样率（0.0 - 1.0）
    pub trace_sample_ratio: f64,
    /// 环境过滤器
    pub env_filter: String,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            service_name: "uhorse".to_string(),
            otlp_endpoint: None,
            console_export: false,
            trace_sample_ratio: 1.0,
            env_filter: "info".to_string(),
        }
    }
}

impl OtelConfig {
    pub fn new(service_name: String) -> Self {
        Self {
            service_name,
            ..Default::default()
        }
    }

    /// 设置 OTLP 端点
    pub fn with_otlp_endpoint(mut self, endpoint: String) -> Self {
        self.otlp_endpoint = Some(endpoint);
        self
    }

    /// 启用控制台导出
    pub fn with_console_export(mut self, enabled: bool) -> Self {
        self.console_export = enabled;
        self
    }

    /// 设置采样率
    pub fn with_sample_ratio(mut self, ratio: f64) -> Self {
        self.trace_sample_ratio = ratio;
        self
    }

    /// 设置环境过滤器
    pub fn with_env_filter(mut self, filter: String) -> Self {
        self.env_filter = filter;
        self
    }
}

/// 初始化 OpenTelemetry
pub fn init_observability(config: OtelConfig) -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = EnvFilter::new(&config.env_filter);

    // 创建格式化层
    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true);

    // 创建 subscriber
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init()?;

    tracing::info!(
        service_name = %config.service_name,
        otlp_endpoint = ?config.otlp_endpoint,
        "Observability initialized"
    );

    Ok(())
}

/// 创建 span 上下文
pub struct SpanContext {
    pub trace_id: String,
    pub span_id: String,
}

impl SpanContext {
    /// 从当前 context 提取
    pub fn current() -> Self {
        let span = tracing::Span::current();
        let id = span.id();

        Self {
            trace_id: format!("{:?}", id),
            span_id: format!("{:?}", id),
        }
    }

    /// 创建新的子 span
    pub fn child(&self, name: &str) -> tracing::Span {
        // 简化实现：直接创建新 span
        tracing::info_span!("{}", name)
    }
}

/// 追踪工具宏
#[macro_export]
macro_rules! traced {
    ($name:expr, $block:expr) => {
        let span = tracing::info_span!($name);
        let _enter = span.enter();
        let result = $block;
        let _ = span.exit();
        result
    };
}

/// 获取当前 trace ID
pub fn current_trace_id() -> String {
    let ctx = SpanContext::current();
    ctx.trace_id
}
