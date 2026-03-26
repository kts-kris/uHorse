//! # uHorse Node Desktop
//!
//! Node 桌面客户端 MVP 入口，当前提供本地宿主 API 与基础健康检查。

mod app_state;
mod config_store;
mod dto;

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};

use anyhow::Context;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use clap::{Parser, Subcommand};
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};
use tracing::info;
use uhorse_node_runtime::{NodeConfig, NodeError, NodeResult, Workspace};

use app_state::DesktopAppState;
use config_store::ConfigStore;
use dto::{
    ApiResponse, DefaultSettingsDto, DesktopCapabilityStatusDto, DesktopLogEntryDto,
    DesktopSettingsDto, DirectoryPickerResponseDto, WorkspaceValidationRequest,
};

const DESKTOP_WEB_DIR_ENV: &str = "UHORSE_NODE_DESKTOP_WEB_DIR";

/// uHorse Node Desktop
#[derive(Parser, Debug)]
#[command(name = "uhorse-node-desktop")]
#[command(author = "uHorse Contributors")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "uHorse Node Desktop MVP", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 工作区路径
    #[arg(short, long, default_value = ".")]
    workspace: String,

    /// 配置文件路径
    #[arg(long, default_value = "node-desktop.toml")]
    config: String,
}

/// 桌面子命令
#[derive(Subcommand, Debug)]
enum Commands {
    /// 检查桌面运行时基础依赖
    Doctor,
    /// 输出桌面客户端默认配置
    PrintConfig,
    /// 启动本地宿主 API
    Serve {
        /// 监听地址
        #[arg(long, default_value = "127.0.0.1:8757")]
        listen: String,
    },
}

fn init_logging() {
    use tracing_subscriber::EnvFilter;

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();

    let args = Args::parse();
    match args.command.unwrap_or(Commands::Doctor) {
        Commands::Doctor => doctor(&args.workspace),
        Commands::PrintConfig => print_config(&args.workspace),
        Commands::Serve { listen } => serve(&args.config, &listen).await,
    }
}

fn doctor(workspace_path: &str) -> anyhow::Result<()> {
    let workspace = Workspace::new(workspace_path)?;
    info!("Desktop workspace resolved: {}", workspace.root().display());
    println!("desktop_ready=true");
    println!("workspace={}", workspace.root().display());
    println!("git_repo={}", workspace.is_git_repo());
    Ok(())
}

fn print_config(workspace_path: &str) -> anyhow::Result<()> {
    let config = NodeConfig {
        workspace_path: workspace_path.to_string(),
        require_git_repo: false,
        ..Default::default()
    };
    println!("{}", serde_json::to_string_pretty(&config)?);
    Ok(())
}

fn build_app(state: DesktopAppState, web_assets_dir: Option<PathBuf>) -> Router {
    let app = Router::new()
        .route("/api/settings", get(get_settings).post(save_settings))
        .route("/api/settings/defaults", get(get_default_settings))
        .route("/api/settings/capabilities", get(get_capabilities))
        .route("/api/settings/notifications/test", post(test_notification))
        .route("/api/workspace/validate", post(validate_workspace))
        .route("/api/workspace/status", get(get_workspace_status))
        .route("/api/workspace/pick", post(pick_workspace))
        .route("/api/runtime/status", get(get_runtime_status))
        .route("/api/runtime/start", post(start_node))
        .route("/api/runtime/stop", post(stop_node))
        .route("/api/versioning/summary", get(get_version_summary))
        .route("/api/logs", get(get_logs))
        .with_state(state)
        .layer(CorsLayer::permissive());

    match web_assets_dir {
        Some(web_assets_dir) => {
            let index_file = web_assets_dir.join("index.html");
            app.route_service("/", ServeFile::new(index_file.clone()))
                .nest_service("/assets", ServeDir::new(web_assets_dir.join("assets")))
                .fallback(get(spa_fallback).with_state(index_file))
        }
        None => app,
    }
}

fn resolve_web_assets_dir() -> Option<PathBuf> {
    let explicit_dir = std::env::var_os(DESKTOP_WEB_DIR_ENV).map(PathBuf::from);
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    let current_dir = std::env::current_dir().ok();

    resolve_web_assets_dir_from(explicit_dir, exe_dir, current_dir)
}

fn resolve_web_assets_dir_from(
    explicit_dir: Option<PathBuf>,
    exe_dir: Option<PathBuf>,
    current_dir: Option<PathBuf>,
) -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path) = explicit_dir {
        candidates.push(path);
    }
    if let Some(path) = exe_dir {
        candidates.push(path.join("web"));
        if let Some(parent) = path.parent() {
            candidates.push(parent.join("web"));
        }
    }
    if let Some(path) = current_dir {
        candidates.push(path.join("apps").join("node-desktop-web").join("dist"));
    }

    candidates.into_iter().find(|path| is_web_assets_dir(path))
}

fn is_web_assets_dir(path: &Path) -> bool {
    path.join("index.html").is_file() && path.join("assets").is_dir()
}

async fn spa_fallback(State(index_file): State<PathBuf>) -> impl IntoResponse {
    match tokio::fs::read_to_string(&index_file).await {
        Ok(content) => (StatusCode::OK, Html(content)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to serve static asset: {}", error),
        )
            .into_response(),
    }
}

async fn serve(config_path: &str, listen: &str) -> anyhow::Result<()> {
    let state = DesktopAppState::new(ConfigStore::new(config_path))?;
    let web_assets_dir = resolve_web_assets_dir();

    if let Some(path) = web_assets_dir.as_ref() {
        info!("Node Desktop web assets: {}", path.display());
    } else {
        info!("Node Desktop web assets not found, serving API only");
    }

    let app = build_app(state, web_assets_dir);
    let address: SocketAddr = listen
        .parse()
        .with_context(|| format!("Invalid listen address: {}", listen))?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    info!("Node Desktop API listening on http://{}", address);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn get_settings(
    State(state): State<DesktopAppState>,
) -> Json<ApiResponse<DesktopSettingsDto>> {
    Json(ApiResponse::success(state.get_settings().await))
}

async fn get_default_settings(
    State(state): State<DesktopAppState>,
) -> Json<ApiResponse<DefaultSettingsDto>> {
    Json(ApiResponse::success(state.default_settings().await))
}

async fn get_capabilities(
    State(state): State<DesktopAppState>,
) -> Json<ApiResponse<DesktopCapabilityStatusDto>> {
    Json(ApiResponse::success(state.capability_status().await))
}

async fn save_settings(
    State(state): State<DesktopAppState>,
    Json(payload): Json<DesktopSettingsDto>,
) -> impl IntoResponse {
    into_api_response(state.save_settings(payload).await)
}

async fn test_notification(State(state): State<DesktopAppState>) -> impl IntoResponse {
    into_api_response(state.test_notification().await)
}

async fn validate_workspace(
    State(state): State<DesktopAppState>,
    Json(payload): Json<WorkspaceValidationRequest>,
) -> Json<ApiResponse<dto::WorkspaceValidationDto>> {
    Json(ApiResponse::success(
        state
            .validate_workspace(payload.workspace_path, payload.require_git_repo)
            .await,
    ))
}

async fn get_workspace_status(
    State(state): State<DesktopAppState>,
) -> Json<ApiResponse<dto::DesktopWorkspaceStatusDto>> {
    Json(ApiResponse::success(state.workspace_status().await))
}

async fn pick_workspace(State(state): State<DesktopAppState>) -> impl IntoResponse {
    match state.pick_workspace().await {
        Ok(path) => (
            StatusCode::OK,
            Json(ApiResponse::success(DirectoryPickerResponseDto { path })),
        ),
        Err(error) => {
            let status = status_code_for_error(&error);
            (status, Json(ApiResponse::error(error.to_string())))
        }
    }
}

async fn get_runtime_status(State(state): State<DesktopAppState>) -> impl IntoResponse {
    into_api_response(state.runtime_status().await)
}

async fn start_node(State(state): State<DesktopAppState>) -> impl IntoResponse {
    into_api_response(state.start_node().await)
}

async fn stop_node(State(state): State<DesktopAppState>) -> impl IntoResponse {
    into_api_response(state.stop_node().await)
}

async fn get_version_summary(
    State(state): State<DesktopAppState>,
) -> Json<ApiResponse<dto::DesktopVersionSummaryDto>> {
    Json(ApiResponse::success(state.version_summary().await))
}

async fn get_logs(
    State(state): State<DesktopAppState>,
) -> Json<ApiResponse<Vec<DesktopLogEntryDto>>> {
    Json(ApiResponse::success(state.logs().await))
}

fn into_api_response<T: serde::Serialize>(
    result: NodeResult<T>,
) -> (StatusCode, Json<ApiResponse<T>>) {
    match result {
        Ok(data) => (StatusCode::OK, Json(ApiResponse::success(data))),
        Err(error) => {
            let status = status_code_for_error(&error);
            (status, Json(ApiResponse::error(error.to_string())))
        }
    }
}

fn status_code_for_error(error: &NodeError) -> StatusCode {
    match error {
        NodeError::Workspace(_) | NodeError::Config(_) => StatusCode::BAD_REQUEST,
        NodeError::Permission(_) => StatusCode::FORBIDDEN,
        NodeError::Connection(_) => StatusCode::BAD_GATEWAY,
        NodeError::Execution(_) => StatusCode::CONFLICT,
        NodeError::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
        NodeError::Protocol(_)
        | NodeError::Io(_)
        | NodeError::Serialization(_)
        | NodeError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Request},
    };
    use serde_json::Value;
    use tempfile::TempDir;
    use tower::util::ServiceExt;

    fn create_state(temp: &TempDir) -> DesktopAppState {
        DesktopAppState::new(ConfigStore::new(temp.path().join("node-desktop.toml"))).unwrap()
    }

    async fn get_json(app: Router, path: &str) -> (StatusCode, Value) {
        let request = Request::builder()
            .method("GET")
            .uri(path)
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json = serde_json::from_slice(&body).unwrap();
        (status, json)
    }

    async fn get_text(app: Router, path: &str) -> (StatusCode, String, Option<String>) {
        let request = Request::builder()
            .method("GET")
            .uri(path)
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        (status, text, content_type)
    }

    #[test]
    fn test_resolve_web_assets_dir_from_package_layout() {
        let temp = TempDir::new().unwrap();
        let package_root = temp.path().join("package");
        let bin_dir = package_root.join("bin");
        let web_dir = package_root.join("web");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::create_dir_all(web_dir.join("assets")).unwrap();
        std::fs::write(web_dir.join("index.html"), "<html>desktop</html>").unwrap();

        let resolved = resolve_web_assets_dir_from(None, Some(bin_dir), None);
        assert_eq!(resolved.unwrap(), web_dir);
    }

    #[test]
    fn test_resolve_web_assets_dir_from_repo_layout() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().join("repo");
        let dist_dir = repo_root.join("apps").join("node-desktop-web").join("dist");
        std::fs::create_dir_all(dist_dir.join("assets")).unwrap();
        std::fs::write(dist_dir.join("index.html"), "<html>desktop</html>").unwrap();

        let resolved = resolve_web_assets_dir_from(None, None, Some(repo_root));
        assert_eq!(resolved.unwrap(), dist_dir);
    }

    #[tokio::test]
    async fn test_build_app_serves_packaged_web_assets() {
        let temp = TempDir::new().unwrap();
        let web_dir = temp.path().join("web");
        std::fs::create_dir_all(web_dir.join("assets")).unwrap();
        std::fs::write(
            web_dir.join("index.html"),
            "<html><body><div id=\"root\">desktop-ui</div></body></html>",
        )
        .unwrap();
        std::fs::write(
            web_dir.join("assets").join("index.js"),
            "console.log('ok');",
        )
        .unwrap();

        let app = build_app(create_state(&temp), Some(web_dir));

        let (status, text, content_type) = get_text(app.clone(), "/").await;
        assert_eq!(status, StatusCode::OK);
        assert!(text.contains("desktop-ui"));
        assert!(content_type.unwrap_or_default().contains("text/html"));

        let (status, text, _) = get_text(app, "/dashboard").await;
        assert_eq!(status, StatusCode::OK);
        assert!(text.contains("desktop-ui"));
    }

    #[tokio::test]
    async fn test_build_app_preserves_api_routes_with_web_assets() {
        let temp = TempDir::new().unwrap();
        let web_dir = temp.path().join("web");
        std::fs::create_dir_all(web_dir.join("assets")).unwrap();
        std::fs::write(
            web_dir.join("index.html"),
            "<html><body>desktop-ui</body></html>",
        )
        .unwrap();
        std::fs::write(
            web_dir.join("assets").join("index.js"),
            "console.log('ok');",
        )
        .unwrap();

        let app = build_app(create_state(&temp), Some(web_dir));
        let (status, body) = get_json(app, "/api/settings/defaults").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], Value::Bool(true));
        assert!(body["data"]["suggested_name"].is_string());
    }
}
