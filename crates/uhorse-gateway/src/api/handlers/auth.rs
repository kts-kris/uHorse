//! # Auth Handlers
//!
//! 认证端点处理器。

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use std::sync::Arc;

use crate::api::types::*;
use crate::http::HttpState;

/// 登录
pub async fn login(
    State(state): State<Arc<HttpState>>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    match state.auth.login(&req.username, &req.password).await {
        Some(result) => {
            let response = TokenResponse {
                access_token: result.access_token,
                refresh_token: result.refresh_token,
                expires_in: result.expires_in,
                token_type: "Bearer".to_string(),
            };
            (StatusCode::OK, Json(ApiResponse::success(response)))
        }
        None => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<TokenResponse>::error(
                "UNAUTHORIZED",
                "Invalid username or password",
            )),
        ),
    }
}

/// 登出
pub async fn logout(
    State(state): State<Arc<HttpState>>,
    Json(req): Json<RefreshTokenRequest>,
) -> impl IntoResponse {
    state.auth.logout(&req.refresh_token).await;
    (StatusCode::OK, Json(ApiResponse::<()>::success(())))
}

/// 刷新令牌
pub async fn refresh_token(
    State(state): State<Arc<HttpState>>,
    Json(req): Json<RefreshTokenRequest>,
) -> impl IntoResponse {
    match state.auth.refresh_token(&req.refresh_token).await {
        Some(result) => {
            let response = TokenResponse {
                access_token: result.access_token,
                refresh_token: result.refresh_token,
                expires_in: result.expires_in,
                token_type: "Bearer".to_string(),
            };
            (StatusCode::OK, Json(ApiResponse::success(response)))
        }
        None => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<TokenResponse>::error(
                "UNAUTHORIZED",
                "Invalid or expired refresh token",
            )),
        ),
    }
}
