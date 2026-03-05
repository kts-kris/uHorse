//! # API Routes
//!
//! 定义 REST API 路由。

use axum::{
    routing::{delete, get, post, put, Router},
    Router as AxumRouter,
};
use std::sync::Arc;

use super::handlers;
use super::types::*;
use crate::http::HttpState;

/// 创建 API 路由器
pub fn create_api_router(state: Arc<HttpState>) -> AxumRouter<Arc<HttpState>> {
    Router::new()
        // === 健康检查 ===
        .route("/health/live", get(handlers::health::health_live))
        .route("/health/ready", get(handlers::health::health_ready))
        // === 认证 ===
        .route("/api/v1/auth/login", post(handlers::auth::login))
        .route("/api/v1/auth/logout", post(handlers::auth::logout))
        .route("/api/v1/auth/refresh", post(handlers::auth::refresh_token))
        // === Agent 管理 ===
        .route(
            "/api/v1/agents",
            get(handlers::agents::list_agents).post(handlers::agents::create_agent),
        )
        .route(
            "/api/v1/agents/:id",
            get(handlers::agents::get_agent)
                .put(handlers::agents::update_agent)
                .delete(handlers::agents::delete_agent),
        )
        // === Skill 管理 ===
        .route(
            "/api/v1/skills",
            get(handlers::skills::list_skills).post(handlers::skills::create_skill),
        )
        .route(
            "/api/v1/skills/:id",
            get(handlers::skills::get_skill)
                .put(handlers::skills::update_skill)
                .delete(handlers::skills::delete_skill),
        )
        // === Session 管理 ===
        .route("/api/v1/sessions", get(handlers::sessions::list_sessions))
        .route(
            "/api/v1/sessions/:id",
            get(handlers::sessions::get_session).delete(handlers::sessions::delete_session),
        )
        .route(
            "/api/v1/sessions/:id/messages",
            get(handlers::sessions::get_session_messages),
        )
        // === 文件管理 ===
        .route("/api/v1/files/:agent_id", get(handlers::files::list_files))
        .route(
            "/api/v1/files/:agent_id/*path",
            get(handlers::files::get_file).put(handlers::files::update_file),
        )
        // === 通道管理 ===
        .route("/api/v1/channels", get(handlers::channels::list_channels))
        .route(
            "/api/v1/channels/:channel_type",
            get(handlers::channels::get_channel_status),
        )
        .route(
            "/api/v1/channels/:channel_type/enable",
            post(handlers::channels::enable_channel),
        )
        .route(
            "/api/v1/channels/:channel_type/disable",
            post(handlers::channels::disable_channel),
        )
        .route(
            "/api/v1/channels/:channel_type/test",
            post(handlers::channels::test_channel),
        )
        // === 系统信息 ===
        .route(
            "/api/v1/system/info",
            get(handlers::system::get_system_info),
        )
        .route("/api/v1/system/metrics", get(handlers::system::get_metrics))
        // === 技能市场 ===
        .route(
            "/api/v1/marketplace/search",
            get(handlers::marketplace::search_skills),
        )
        .route(
            "/api/v1/marketplace/skills/:id",
            get(handlers::marketplace::get_marketplace_skill),
        )
        .route(
            "/api/v1/marketplace/install/:id",
            post(handlers::marketplace::install_skill),
        )
        // === 原有端点 ===
        .route("/api/v1/info", get(handlers::system::get_info))
        .route("/metrics", get(handlers::system::prometheus_metrics))
        // === 实时通信 ===
        .route("/api/v1/events", get(handlers::streams::sse_events))
        .route("/api/v1/chat/stream", post(handlers::streams::stream_chat))
        .route(
            "/api/v1/connections",
            get(handlers::streams::get_active_connections),
        )
        .route("/ws", get(crate::websocket::handle_upgrade))
}
