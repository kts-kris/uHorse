//! # Skill Handlers
//!
//! Skill 管理端点处理器。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use std::sync::Arc;

use crate::api::types::*;
use crate::http::HttpState;

/// 列出所有 Skills
pub async fn list_skills(
    State(_state): State<Arc<HttpState>>,
    Query(_pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    // TODO: 实现从存储层获取 Skill 列表
    let skills: Vec<SkillDto> = vec![];
    let response = PaginatedResponse::new(skills, 0, 1, 20);
    (StatusCode::OK, Json(ApiResponse::success(response)))
}

/// 获取单个 Skill
pub async fn get_skill(
    State(_state): State<Arc<HttpState>>,
    Path(_id): Path<String>,
) -> impl IntoResponse {
    // TODO: 实现获取 Skill 详情
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse::<SkillDto>::error(
            "NOT_FOUND",
            "Skill not found",
        )),
    )
}

/// 创建 Skill
pub async fn create_skill(
    State(_state): State<Arc<HttpState>>,
    Json(_req): Json<CreateSkillRequest>,
) -> impl IntoResponse {
    // TODO: 实现创建 Skill
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse::<SkillDto>::error(
            "NOT_IMPLEMENTED",
            "Not implemented yet",
        )),
    )
}

/// 更新 Skill
pub async fn update_skill(
    State(_state): State<Arc<HttpState>>,
    Path(_id): Path<String>,
    Json(_req): Json<UpdateSkillRequest>,
) -> impl IntoResponse {
    // TODO: 实现更新 Skill
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse::<SkillDto>::error(
            "NOT_IMPLEMENTED",
            "Not implemented yet",
        )),
    )
}

/// 删除 Skill
pub async fn delete_skill(
    State(_state): State<Arc<HttpState>>,
    Path(_id): Path<String>,
) -> impl IntoResponse {
    // TODO: 实现删除 Skill
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse::<()>::error(
            "NOT_IMPLEMENTED",
            "Not implemented yet",
        )),
    )
}
