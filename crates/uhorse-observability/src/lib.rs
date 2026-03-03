//! # uHorse Observability
//!
//! 可观测性层，提供 tracing、metrics 和审计日志。

pub mod audit;
pub mod health;
pub mod metrics;
pub mod rotation;
pub mod shutdown;
pub mod telemetry;
pub mod tracing_setup;

pub use health::{
    liveness, readiness, CheckResult, CheckerType, HealthCheck, HealthService, HealthStatus,
};
pub use metrics::{
    ApiTimer, AuditFilter, AuditLog, AuditLogger, AuditResult, HealthMetrics, MetricsCollector,
    MetricsExporter, SystemMonitor, ToolTimer,
};
pub use rotation::{LogArchiver, LogRotator, RotationStrategy};
pub use shutdown::{
    GracefulShutdown, ShutdownHandle, ShutdownManager, ShutdownPhase, ShutdownSignal, ShutdownTask,
};
pub use telemetry::{current_trace_id, init_observability, OtelConfig, SpanContext};
pub use tracing_setup::{init_dev_observability, init_full_observability, init_tracing};
