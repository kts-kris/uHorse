//! # Channel Handlers
//!
//! 通道管理端点处理器。

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use std::sync::Arc;

use crate::api::types::*;
use crate::http::HttpState;

/// 列出所有通道状态
pub async fn list_channels(State(_state): State<Arc<HttpState>>) -> impl IntoResponse {
    // TODO: 实现从通道管理器获取状态
    let channels: Vec<ChannelStatusDto> = vec![
        ChannelStatusDto {
            channel_type: "telegram".to_string(),
            enabled: true,
            running: false,
            connected: false,
            last_activity: None,
            error: None,
        },
        ChannelStatusDto {
            channel_type: "dingtalk".to_string(),
            enabled: false,
            running: false,
            connected: false,
            last_activity: None,
            error: None,
        },
    ];
    (StatusCode::OK, Json(ApiResponse::success(channels)))
}

/// 获取单个通道状态
pub async fn get_channel_status(
    State(_state): State<Arc<HttpState>>,
    Path(_channel_type): Path<String>,
) -> impl IntoResponse {
    // TODO: 实现获取单个通道状态
    let status = ChannelStatusDto {
        channel_type: "telegram".to_string(),
        enabled: true,
        running: false,
        connected: false,
        last_activity: None,
        error: None,
    };
    (StatusCode::OK, Json(ApiResponse::success(status)))
}
