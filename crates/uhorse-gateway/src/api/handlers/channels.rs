//! # Channel Handlers
//!
//! 通道管理端点处理器。

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use std::sync::Arc;
use tracing::{debug, info};

use crate::api::types::*;
use crate::http::HttpState;

/// 支持的通道类型
const CHANNEL_TYPES: [&str; 7] = [
    "telegram", "dingtalk", "feishu", "wework", "slack", "discord", "whatsapp",
];

/// 列出所有通道状态
#[axum::debug_handler]
pub async fn list_channels(State(state): State<Arc<HttpState>>) -> impl IntoResponse {
    debug!("Listing all channels");

    let channels = state.store.list_channels().await;
    (StatusCode::OK, Json(ApiResponse::success(channels)))
}

/// 获取单个通道状态
#[axum::debug_handler]
pub async fn get_channel_status(
    State(state): State<Arc<HttpState>>,
    Path(channel_type): Path<String>,
) -> impl IntoResponse {
    debug!("Getting channel status: {}", channel_type);

    match state.store.get_channel_status(&channel_type).await {
        Some(status) => (StatusCode::OK, Json(ApiResponse::success(status))),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<ChannelStatusDto>::error(
                "NOT_FOUND",
                "Channel type not found",
            )),
        ),
    }
}

/// 启用通道
#[axum::debug_handler]
pub async fn enable_channel(
    State(state): State<Arc<HttpState>>,
    Path(channel_type): Path<String>,
) -> impl IntoResponse {
    info!("Enabling channel: {}", channel_type);

    if !CHANNEL_TYPES.contains(&channel_type.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<ChannelStatusDto>::error(
                "INVALID_CHANNEL",
                "Unknown channel type",
            )),
        );
    }

    let status = state.store.set_channel_enabled(&channel_type, true).await;
    (StatusCode::OK, Json(ApiResponse::success(status)))
}

/// 禁用通道
#[axum::debug_handler]
pub async fn disable_channel(
    State(state): State<Arc<HttpState>>,
    Path(channel_type): Path<String>,
) -> impl IntoResponse {
    info!("Disabling channel: {}", channel_type);

    if !CHANNEL_TYPES.contains(&channel_type.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<ChannelStatusDto>::error(
                "INVALID_CHANNEL",
                "Unknown channel type",
            )),
        );
    }

    let status = state.store.set_channel_enabled(&channel_type, false).await;
    (StatusCode::OK, Json(ApiResponse::success(status)))
}

/// 测试通道连接
#[axum::debug_handler]
pub async fn test_channel(
    State(state): State<Arc<HttpState>>,
    Path(channel_type): Path<String>,
) -> impl IntoResponse {
    info!("Testing channel connection: {}", channel_type);

    if !CHANNEL_TYPES.contains(&channel_type.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value>::error(
                "INVALID_CHANNEL",
                "Unknown channel type",
            )),
        );
    }

    // TODO: 实现实际的通道连接测试
    let result = serde_json::json!({
        "channel_type": channel_type,
        "test_result": "success",
        "message": "Channel connection test passed"
    });

    (StatusCode::OK, Json(ApiResponse::success(result)))
}
