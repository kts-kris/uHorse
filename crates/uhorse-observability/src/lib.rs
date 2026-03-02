//! # uHorse Observability
//!
//! 可观测性层，提供 tracing、metrics 和审计日志。

pub mod tracing_setup;
pub mod telemetry;
pub mod metrics;
pub mod audit;
pub mod health;
pub mod shutdown;
pub mod rotation;

pub use tracing_setup::{init_tracing, init_dev_observability, init_full_observability};
pub use telemetry::{OtelConfig, init_observability, SpanContext, current_trace_id};
pub use metrics::{
    MetricsCollector, MetricsExporter, ToolTimer, ApiTimer,
    AuditLogger, AuditLog, AuditResult, AuditFilter,
    HealthMetrics, SystemMonitor,
};
pub use health::{
    HealthStatus, HealthCheck, CheckResult,
    HealthService, CheckerType,
    liveness, readiness,
};
pub use shutdown::{
    ShutdownSignal, ShutdownPhase, ShutdownHandle,
    GracefulShutdown, ShutdownManager, ShutdownTask,
};
pub use rotation::{
    LogRotator, LogArchiver, RotationStrategy,
};
