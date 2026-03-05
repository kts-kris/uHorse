//! # Session Handlers
//!
//! Session 管理端点处理器。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use std::sync::Arc;
use tracing::{debug, info};

use crate::api::types::*;
use crate::http::HttpState;

/// 列出所有 Sessions
#[axum::debug_handler]
pub async fn list_sessions(
    State(state): State<Arc<HttpState>>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    debug!(
        "Listing sessions: page={}, per_page={}",
        pagination.page, pagination.per_page
    );

    let (sessions, total) = state
        .store
        .list_sessions(pagination.page, pagination.per_page)
        .await;

    let items: Vec<SessionDto> = sessions.into_iter().map(|s| s.to_dto()).collect();
    let response = PaginatedResponse::new(items, total, pagination.page, pagination.per_page);
    (StatusCode::OK, Json(ApiResponse::success(response)))
}

/// 获取单个 Session
#[axum::debug_handler]
pub async fn get_session(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!("Getting session: {}", id);

    match state.store.get_session(&id).await {
        Some(session) => (StatusCode::OK, Json(ApiResponse::success(session.to_dto()))),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<SessionDto>::error(
                "NOT_FOUND",
                "Session not found",
            )),
        ),
    }
}

/// 删除 Session
#[axum::debug_handler]
pub async fn delete_session(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    info!("Deleting session: {}", id);

    if state.store.delete_session(&id).await {
        (StatusCode::OK, Json(ApiResponse::success(())))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("NOT_FOUND", "Session not found")),
        )
    }
}

/// 获取 Session 消息历史
#[axum::debug_handler]
pub async fn get_session_messages(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    debug!("Getting session messages: session_id={}", id);

    let (messages, total) = state
        .store
        .get_session_messages(&id, pagination.page, pagination.per_page)
        .await;

    let items: Vec<SessionMessageDto> = messages.into_iter().map(|m| m.to_dto()).collect();
    let response = PaginatedResponse::new(items, total, pagination.page, pagination.per_page);
    (StatusCode::OK, Json(ApiResponse::success(response)))
}
