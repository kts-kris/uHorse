//! # Session Handlers
//!
//! Session 管理端点处理器。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use std::sync::Arc;

use crate::api::types::*;
use crate::http::HttpState;

/// 列出所有 Sessions
pub async fn list_sessions(
    State(_state): State<Arc<HttpState>>,
    Query(_pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    // TODO: 实现从存储层获取 Session 列表
    let sessions: Vec<SessionDto> = vec![];
    let response = PaginatedResponse::new(sessions, 0, 1, 20);
    (StatusCode::OK, Json(ApiResponse::success(response)))
}

/// 获取单个 Session
pub async fn get_session(
    State(_state): State<Arc<HttpState>>,
    Path(_id): Path<String>,
) -> impl IntoResponse {
    // TODO: 实现获取 Session 详情
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse::<SessionDto>::error("NOT_FOUND", "Session not found")),
    )
}

/// 删除 Session
pub async fn delete_session(
    State(_state): State<Arc<HttpState>>,
    Path(_id): Path<String>,
) -> impl IntoResponse {
    // TODO: 实现删除 Session
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse::<()>::error("NOT_IMPLEMENTED", "Not implemented yet")),
    )
}

/// 获取 Session 消息历史
pub async fn get_session_messages(
    State(_state): State<Arc<HttpState>>,
    Path(_id): Path<String>,
    Query(_pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    // TODO: 实现获取 Session 消息
    let messages: Vec<SessionMessageDto> = vec![];
    let response = PaginatedResponse::new(messages, 0, 1, 20);
    (StatusCode::OK, Json(ApiResponse::success(response)))
}
