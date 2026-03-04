//! # Marketplace Handlers
//!
//! 技能市场端点处理器。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use std::sync::Arc;

use crate::api::types::*;
use crate::http::HttpState;

/// 搜索技能市场
pub async fn search_skills(
    State(_state): State<Arc<HttpState>>,
    Query(_query): Query<MarketplaceSearchQuery>,
) -> impl IntoResponse {
    // TODO: 实现从技能市场搜索
    let skills: Vec<MarketplaceSkill> = vec![];
    (StatusCode::OK, Json(ApiResponse::success(skills)))
}

/// 获取市场技能详情
pub async fn get_marketplace_skill(
    State(_state): State<Arc<HttpState>>,
    Path(_id): Path<String>,
) -> impl IntoResponse {
    // TODO: 实现获取技能详情
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse::<MarketplaceSkill>::error("NOT_FOUND", "Skill not found in marketplace")),
    )
}

/// 安装技能
pub async fn install_skill(
    State(_state): State<Arc<HttpState>>,
    Path(_id): Path<String>,
    Json(_req): Json<InstallSkillRequest>,
) -> impl IntoResponse {
    // TODO: 实现安装技能
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse::<SkillDto>::error("NOT_IMPLEMENTED", "Not implemented yet")),
    )
}
