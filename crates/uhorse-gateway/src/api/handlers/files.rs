//! # File Handlers
//!
//! 文件管理端点处理器。

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use std::sync::Arc;

use crate::api::types::*;
use crate::http::HttpState;

/// 列出 Agent 工作空间文件
pub async fn list_files(
    State(_state): State<Arc<HttpState>>,
    Path(_agent_id): Path<String>,
) -> impl IntoResponse {
    // TODO: 实现列出文件
    let files: Vec<FileInfo> = vec![];
    (StatusCode::OK, Json(ApiResponse::success(files)))
}

/// 获取文件内容
pub async fn get_file(
    State(_state): State<Arc<HttpState>>,
    Path((_agent_id, _path)): Path<(String, String)>,
) -> impl IntoResponse {
    // TODO: 实现获取文件内容
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse::<FileContent>::error("NOT_FOUND", "File not found")),
    )
}

/// 更新文件内容
pub async fn update_file(
    State(_state): State<Arc<HttpState>>,
    Path((_agent_id, _path)): Path<(String, String)>,
    _body: Bytes,
) -> impl IntoResponse {
    // TODO: 实现更新文件
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse::<()>::error("NOT_IMPLEMENTED", "Not implemented yet")),
    )
}
