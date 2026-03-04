//! # Agent Handlers
//!
//! Agent 管理端点处理器。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use std::sync::Arc;

use crate::api::types::*;
use crate::http::HttpState;

/// 列出所有 Agents
pub async fn list_agents(
    State(_state): State<Arc<HttpState>>,
    Query(_pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    // TODO: 实现从存储层获取 Agent 列表
    let agents: Vec<AgentDto> = vec![];
    let response = PaginatedResponse::new(agents, 0, 1, 20);
    (StatusCode::OK, Json(ApiResponse::success(response)))
}

/// 获取单个 Agent
pub async fn get_agent(
    State(_state): State<Arc<HttpState>>,
    Path(_id): Path<String>,
) -> impl IntoResponse {
    // TODO: 实现获取 Agent 详情
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse::<AgentDto>::error("NOT_FOUND", "Agent not found")),
    )
}

/// 创建 Agent
pub async fn create_agent(
    State(_state): State<Arc<HttpState>>,
    Json(_req): Json<CreateAgentRequest>,
) -> impl IntoResponse {
    // TODO: 实现创建 Agent
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse::<AgentDto>::error("NOT_IMPLEMENTED", "Not implemented yet")),
    )
}

/// 更新 Agent
pub async fn update_agent(
    State(_state): State<Arc<HttpState>>,
    Path(_id): Path<String>,
    Json(_req): Json<UpdateAgentRequest>,
) -> impl IntoResponse {
    // TODO: 实现更新 Agent
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse::<AgentDto>::error("NOT_IMPLEMENTED", "Not implemented yet")),
    )
}

/// 删除 Agent
pub async fn delete_agent(
    State(_state): State<Arc<HttpState>>,
    Path(_id): Path<String>,
) -> impl IntoResponse {
    // TODO: 实现删除 Agent
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse::<()>::error("NOT_IMPLEMENTED", "Not implemented yet")),
    )
}
