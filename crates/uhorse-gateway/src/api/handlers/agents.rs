//! # Agent Handlers
//!
//! Agent 管理端点处理器。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use std::sync::Arc;
use tracing::{debug, info};

use crate::api::types::*;
use crate::http::HttpState;
use crate::store::Agent;

/// 列出所有 Agents
#[axum::debug_handler]
pub async fn list_agents(
    State(state): State<Arc<HttpState>>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    debug!(
        "Listing agents: page={}, per_page={}",
        pagination.page, pagination.per_page
    );

    let (agents, total) = state
        .store
        .list_agents(pagination.page, pagination.per_page)
        .await;

    let items: Vec<AgentDto> = agents.into_iter().map(|a| a.to_dto()).collect();
    let response = PaginatedResponse::new(items, total, pagination.page, pagination.per_page);

    (StatusCode::OK, Json(ApiResponse::success(response)))
}

/// 获取单个 Agent
#[axum::debug_handler]
pub async fn get_agent(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!("Getting agent: {}", id);

    match state.store.get_agent(&id).await {
        Some(agent) => (StatusCode::OK, Json(ApiResponse::success(agent.to_dto()))),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<AgentDto>::error(
                "NOT_FOUND",
                "Agent not found",
            )),
        ),
    }
}

/// 创建 Agent
#[axum::debug_handler]
pub async fn create_agent(
    State(state): State<Arc<HttpState>>,
    Json(req): Json<CreateAgentRequest>,
) -> impl IntoResponse {
    info!("Creating agent: {}", req.name);

    let agent = Agent::new(req);
    let dto = agent.to_dto();
    state.store.create_agent(agent).await;

    (StatusCode::CREATED, Json(ApiResponse::success(dto)))
}

/// 更新 Agent
#[axum::debug_handler]
pub async fn update_agent(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateAgentRequest>,
) -> impl IntoResponse {
    info!("Updating agent: {}", id);

    match state.store.update_agent(&id, req).await {
        Some(agent) => (StatusCode::OK, Json(ApiResponse::success(agent.to_dto()))),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<AgentDto>::error(
                "NOT_FOUND",
                "Agent not found",
            )),
        ),
    }
}

/// 删除 Agent
#[axum::debug_handler]
pub async fn delete_agent(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    info!("Deleting agent: {}", id);

    if state.store.delete_agent(&id).await {
        (StatusCode::OK, Json(ApiResponse::success(())))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("NOT_FOUND", "Agent not found")),
        )
    }
}

/// 启动 Agent
#[axum::debug_handler]
pub async fn start_agent(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    info!("Starting agent: {}", id);

    // TODO: 实现实际的 Agent 启动逻辑
    match state.store.get_agent(&id).await {
        Some(agent) => (
            StatusCode::OK,
            Json(ApiResponse::success(serde_json::json!({
                "id": agent.id,
                "status": "running"
            }))),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<serde_json::Value>::error(
                "NOT_FOUND",
                "Agent not found",
            )),
        ),
    }
}

/// 停止 Agent
#[axum::debug_handler]
pub async fn stop_agent(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    info!("Stopping agent: {}", id);

    // TODO: 实现实际的 Agent 停止逻辑
    match state.store.get_agent(&id).await {
        Some(agent) => (
            StatusCode::OK,
            Json(ApiResponse::success(serde_json::json!({
                "id": agent.id,
                "status": "stopped"
            }))),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<serde_json::Value>::error(
                "NOT_FOUND",
                "Agent not found",
            )),
        ),
    }
}
