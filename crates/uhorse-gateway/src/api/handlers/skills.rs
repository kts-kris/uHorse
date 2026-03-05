//! # Skill Handlers
//!
//! Skill 管理端点处理器。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use std::sync::Arc;
use tracing::{debug, info};

use crate::api::types::*;
use crate::http::HttpState;
use crate::store::Skill;

/// 列出所有 Skills
#[axum::debug_handler]
pub async fn list_skills(
    State(state): State<Arc<HttpState>>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    debug!(
        "Listing skills: page={}, per_page={}",
        pagination.page, pagination.per_page
    );

    let (skills, total) = state
        .store
        .list_skills(pagination.page, pagination.per_page)
        .await;

    let items: Vec<SkillDto> = skills.into_iter().map(|s| s.to_dto()).collect();
    let response = PaginatedResponse::new(items, total, pagination.page, pagination.per_page);
    (StatusCode::OK, Json(ApiResponse::success(response)))
}

/// 获取单个 Skill
#[axum::debug_handler]
pub async fn get_skill(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!("Getting skill: {}", id);

    match state.store.get_skill(&id).await {
        Some(skill) => (StatusCode::OK, Json(ApiResponse::success(skill.to_dto()))),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<SkillDto>::error(
                "NOT_FOUND",
                "Skill not found",
            )),
        ),
    }
}

/// 创建 Skill
#[axum::debug_handler]
pub async fn create_skill(
    State(state): State<Arc<HttpState>>,
    Json(req): Json<CreateSkillRequest>,
) -> impl IntoResponse {
    info!("Creating skill: {}", req.name);

    let skill = Skill::new(req);
    let dto = skill.to_dto();
    state.store.create_skill(skill).await;

    (StatusCode::CREATED, Json(ApiResponse::success(dto)))
}

/// 更新 Skill
#[axum::debug_handler]
pub async fn update_skill(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateSkillRequest>,
) -> impl IntoResponse {
    info!("Updating skill: {}", id);

    match state.store.update_skill(&id, req).await {
        Some(skill) => (StatusCode::OK, Json(ApiResponse::success(skill.to_dto()))),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<SkillDto>::error(
                "NOT_FOUND",
                "Skill not found",
            )),
        ),
    }
}

/// 删除 Skill
#[axum::debug_handler]
pub async fn delete_skill(
    State(state): State<Arc<HttpState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    info!("Deleting skill: {}", id);

    if state.store.delete_skill(&id).await {
        (StatusCode::OK, Json(ApiResponse::success(())))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("NOT_FOUND", "Skill not found")),
        )
    }
}
