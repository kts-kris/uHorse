//! # uHorse Node Desktop
//!
//! Node 桌面客户端 MVP 入口，当前提供本地宿主 API 与基础健康检查。

mod app_state;
mod config_store;
mod dto;

use std::net::SocketAddr;

use anyhow::Context;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use clap::{Parser, Subcommand};
use tower_http::cors::CorsLayer;
use tracing::info;
use uhorse_node_runtime::{NodeConfig, NodeError, NodeResult, Workspace};

use app_state::DesktopAppState;
use config_store::ConfigStore;
use dto::{
    ApiResponse, DefaultSettingsDto, DesktopCapabilityStatusDto, DesktopLogEntryDto,
    DesktopSettingsDto, DirectoryPickerResponseDto, WorkspaceValidationRequest,
};

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

async fn serve(config_path: &str, listen: &str) -> anyhow::Result<()> {
    let state = DesktopAppState::new(ConfigStore::new(config_path))?;
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

async fn test_notification(
    State(state): State<DesktopAppState>,
) -> impl IntoResponse {
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

async fn pick_workspace(
    State(state): State<DesktopAppState>,
) -> impl IntoResponse {
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

async fn get_runtime_status(
    State(state): State<DesktopAppState>,
) -> impl IntoResponse {
    into_api_response(state.runtime_status().await)
}

async fn start_node(
    State(state): State<DesktopAppState>,
) -> impl IntoResponse {
    into_api_response(state.start_node().await)
}

async fn stop_node(
    State(state): State<DesktopAppState>,
) -> impl IntoResponse {
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

fn into_api_response<T: serde::Serialize>(result: NodeResult<T>) -> (StatusCode, Json<ApiResponse<T>>) {
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
        NodeError::Protocol(_) | NodeError::Io(_) | NodeError::Serialization(_) | NodeError::Internal(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
