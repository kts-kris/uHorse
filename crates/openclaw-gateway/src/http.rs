//! # HTTP API 处理器
//!
//! 提供 REST API 端点。

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// HTTP 处理器状态
#[derive(Debug, Clone)]
pub struct HttpState {
    // TODO: 添加需要共享的状态
}

/// 创建 HTTP 路由
pub fn create_router() -> Router {
    let state = Arc::new(HttpState {});

    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/info", get(get_info))
        .with_state(state)
}

/// 健康检查
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// 获取信息
async fn get_info(State(_state): State<Arc<HttpState>>) -> impl IntoResponse {
    Json(serde_json::json!({
        "name": "OpenClaw AI Gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Multi-channel AI Gateway Framework",
    }))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}
