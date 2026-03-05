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
use tracing::{debug, info};

use crate::api::types::*;
use crate::http::HttpState;

/// 列出 Agent 工作空间文件
#[axum::debug_handler]
pub async fn list_files(
    State(state): State<Arc<HttpState>>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    debug!("Listing files for agent: {}", agent_id);

    let files = state.store.list_files(&agent_id).await;
    (StatusCode::OK, Json(ApiResponse::success(files)))
}

/// 获取文件内容
#[axum::debug_handler]
pub async fn get_file(
    State(state): State<Arc<HttpState>>,
    Path((agent_id, path)): Path<(String, String)>,
) -> impl IntoResponse {
    debug!("Getting file: agent_id={}, path={}", agent_id, path);

    match state.store.get_file(&agent_id, &path).await {
        Some(file) => {
            let size = file.content.len() as u64;
            let content = FileContent {
                path: file.path,
                content: file.content,
                size,
            };
            (StatusCode::OK, Json(ApiResponse::success(content)))
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<FileContent>::error(
                "NOT_FOUND",
                "File not found",
            )),
        ),
    }
}

/// 更新文件内容
#[axum::debug_handler]
pub async fn update_file(
    State(state): State<Arc<HttpState>>,
    Path((agent_id, path)): Path<(String, String)>,
    body: Bytes,
) -> impl IntoResponse {
    info!("Updating file: agent_id={}, path={}", agent_id, path);

    let content = String::from_utf8_lossy(&body).to_string();
    let file = state.store.save_file(&agent_id, &path, content).await;

    let info = file.to_info();
    (StatusCode::OK, Json(ApiResponse::success(info)))
}

/// 创建文件
#[axum::debug_handler]
pub async fn create_file(
    State(state): State<Arc<HttpState>>,
    Path((agent_id, path)): Path<(String, String)>,
    Json(req): Json<CreateFileRequest>,
) -> impl IntoResponse {
    info!("Creating file: agent_id={}, path={}", agent_id, path);

    let file = state.store.save_file(&agent_id, &path, req.content).await;
    let info = file.to_info();

    (StatusCode::CREATED, Json(ApiResponse::success(info)))
}

/// 删除文件
#[axum::debug_handler]
pub async fn delete_file(
    State(state): State<Arc<HttpState>>,
    Path((agent_id, path)): Path<(String, String)>,
) -> impl IntoResponse {
    info!("Deleting file: agent_id={}, path={}", agent_id, path);

    if state.store.delete_file(&agent_id, &path).await {
        (StatusCode::OK, Json(ApiResponse::success(())))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("NOT_FOUND", "File not found")),
        )
    }
}
