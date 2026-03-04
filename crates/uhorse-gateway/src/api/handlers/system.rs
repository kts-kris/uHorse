//! # System Handlers
//!
//! 系统信息端点处理器。

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde_json::json;
use std::sync::Arc;

use crate::api::types::*;
use crate::http::HttpState;

/// 获取系统信息
pub async fn get_system_info(State(_state): State<Arc<HttpState>>) -> impl IntoResponse {
    let info = SystemInfo {
        name: "uHorse AI Gateway".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs: 0, // TODO: 实现运行时间计算
        rust_version: "1.75+".to_string(),
        channels_count: 7,
        agents_count: 0,
        active_sessions: 0,
    };
    (StatusCode::OK, Json(ApiResponse::success(info)))
}

/// 获取系统指标
pub async fn get_metrics(State(_state): State<Arc<HttpState>>) -> impl IntoResponse {
    let metrics = SystemMetrics {
        total_messages: 0,
        messages_today: 0,
        total_requests: 0,
        total_errors: 0,
        avg_response_time_ms: 0.0,
        memory_usage_bytes: 0,
    };
    (StatusCode::OK, Json(ApiResponse::success(metrics)))
}

/// Prometheus 格式指标
pub async fn prometheus_metrics(State(_state): State<Arc<HttpState>>) -> impl IntoResponse {
    // TODO: 实现真正的 Prometheus 指标收集
    (
        StatusCode::OK,
        format!(
            r#"# HELP uhorse_version Application version
# TYPE uhorse_version gauge
uhorse_version{{version="{}"}} 1
# HELP uhorse_uptime_seconds Application uptime
# TYPE uhorse_uptime_seconds counter
uhorse_uptime_seconds 0
"#,
            env!("CARGO_PKG_VERSION")
        ),
    )
}

/// 获取服务信息（原有端点）
pub async fn get_info(State(_state): State<Arc<HttpState>>) -> impl IntoResponse {
    Json(json!({
        "name": "uHorse AI Gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Multi-channel AI Gateway Framework",
    }))
}
