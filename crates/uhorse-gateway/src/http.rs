//! # HTTP API 处理器
//!
//! 提供 REST API 端点。

use axum::{
    extract::State,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::api;
use crate::auth::AuthService;
use crate::store::MemoryStore;
use crate::websocket::ConnectionManager;

/// HTTP 处理器状态
#[derive(Debug, Clone)]
pub struct HttpState {
    /// 认证服务
    pub auth: Arc<AuthService>,
    /// 内存存储
    pub store: Arc<MemoryStore>,
    /// WebSocket 连接管理器
    pub ws_manager: Arc<ConnectionManager>,
}

impl HttpState {
    /// 创建新的 HTTP 状态
    pub fn new() -> Self {
        Self {
            auth: Arc::new(AuthService::default()),
            store: Arc::new(MemoryStore::new()),
            ws_manager: Arc::new(ConnectionManager::new()),
        }
    }
}

impl Default for HttpState {
    fn default() -> Self {
        Self::new()
    }
}

/// 创建 HTTP 路由
pub fn create_router() -> Router {
    let state = Arc::new(HttpState::new());

    // 合并 API 路由和基础路由
    let api_router = api::create_api_router(state.clone());

    Router::new()
        .route("/health", get(health_check))
        .merge(api_router)
        .with_state(state)
}

/// 健康检查
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}
