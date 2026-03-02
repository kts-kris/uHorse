//! # OpenClaw AI Gateway
//!
//! 多渠道 AI 网关框架主程序。

use clap::Parser;
use tokio::signal;
use tracing::{info, error};

/// OpenClaw AI Gateway
#[derive(Parser, Debug)]
#[command(name = "openclaw")]
#[command(author = "OpenClaw Contributors")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Multi-channel AI Gateway Framework", long_about = None)]
struct Args {
    /// 配置文件路径
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// 日志级别
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// 是否启用开发模式
    #[arg(long, default_value = "false")]
    dev: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // 初始化日志
    init_logging(&args.log_level)?;

    info!("🦀 OpenClaw AI Gateway v{} starting...", env!("CARGO_PKG_VERSION"));

    // 加载配置
    info!("📄 Loading config from: {}", args.config);
    let config = load_config(&args.config)?;

    // TODO: 初始化各个组件
    info!("🔌 Initializing gateway...");
    info!("💾 Initializing storage...");
    info!("📱 Initializing channels...");
    info!("🛠️  Initializing tools...");
    info!("🔒 Initializing security...");
    info!("⏰ Initializing scheduler...");

    // 启动服务器
    info!("🚀 Starting server on {}:{}",
        config.server.host, config.server.port);

    // 等待关闭信号
    shutdown_signal().await;

    info!("👋 Shutting down gracefully...");

    Ok(())
}

/// 初始化日志
fn init_logging(level: &str) -> anyhow::Result<()> {
    use tracing_subscriber::{EnvFilter, fmt};

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    Ok(())
}

/// 配置结构
#[derive(Debug, serde::Deserialize)]
struct Config {
    server: ServerConfig,
    #[serde(default)]
    channels: ChannelsConfig,
    #[serde(default)]
    database: DatabaseConfig,
    #[serde(default)]
    security: SecurityConfig,
}

#[derive(Debug, serde::Deserialize)]
struct ServerConfig {
    #[serde(default = "default_host")]
    host: String,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default = "default_max_connections")]
    max_connections: usize,
}

fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_max_connections() -> usize { 1000 }

#[derive(Debug, serde::Deserialize, Default)]
struct ChannelsConfig {
    #[serde(default)]
    enabled: Vec<String>,
}

#[derive(Debug, serde::Deserialize, Default)]
struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    path: String,
}

fn default_db_path() -> String { "./data/openclaw.db".to_string() }

#[derive(Debug, serde::Deserialize, Default)]
struct SecurityConfig {
    #[serde(default)]
    jwt_secret: Option<String>,
    #[serde(default = "default_token_expiry")]
    token_expiry: u64,
}

fn default_token_expiry() -> u64 { 86400 }

/// 加载配置文件
fn load_config(path: &str) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

/// 等待关闭信号
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C");
        }
        _ = terminate => {
            info!("Received terminate signal");
        }
    }
}
