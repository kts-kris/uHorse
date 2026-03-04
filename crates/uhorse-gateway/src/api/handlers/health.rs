//! # Health Check Handlers
//!
//! 健康检查端点处理器。

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde_json::json;
use std::sync::Arc;

use crate::http::HttpState;

/// 存活性检查
pub async fn health_live() -> impl IntoResponse {
    Json(json!({
        "status": "alive",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// 就绪性检查
pub async fn health_ready(State(_state): State<Arc<HttpState>>) -> impl IntoResponse {
    // TODO: 检查数据库连接、通道状态等
    Json(json!({
        "status": "ready",
        "checks": {
            "database": "ok",
            "channels": "ok",
        }
    }))
}
