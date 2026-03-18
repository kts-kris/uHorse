//! # uHorse Hub
//!
//! 云端中枢二进制程序，负责管理节点、调度任务和路由消息。

use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};

use uhorse_hub::{create_router, Hub, HubConfig, WebState};

/// uHorse Hub - 云端中枢
#[derive(Parser, Debug)]
#[command(name = "uhorse-hub")]
#[command(author = "uHorse Contributors")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "uHorse Cloud Hub - Node management and task orchestration", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 配置文件路径
    #[arg(short, long, default_value = "hub.toml")]
    config: String,

    /// 日志级别
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// 监听地址
    #[arg(short, long, default_value = "0.0.0.0")]
    host: String,

    /// 监听端口
    #[arg(short, long, default_value = "8765")]
    port: u16,

    /// Hub ID
    #[arg(long, default_value = "default-hub")]
    hub_id: String,
}

/// 子命令
#[derive(Subcommand, Debug)]
enum Commands {
    /// 生成默认配置文件
    Init {
        /// 输出路径
        #[arg(short, long, default_value = "hub.toml")]
        output: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // 处理子命令
    if let Some(Commands::Init { output }) = args.command {
        return generate_config(&output);
    }

    // 初始化日志
    init_logging(&args.log_level)?;

    info!("🚀 uHorse Hub v{} starting...", env!("CARGO_PKG_VERSION"));

    // 加载或使用默认配置
    let config = load_config(&args.config, &args)?;

    // 创建 Hub
    let (hub, _task_result_rx) = Hub::new(config.clone());

    // 启动 Hub
    hub.start().await?;

    info!(
        "📡 Hub {} listening on {}:{}",
        config.hub_id, config.bind_address, config.port
    );

    // 创建 Web 状态
    let web_state = WebState { hub: Arc::new(hub) };

    // 创建路由
    let app = create_router(web_state);

    // 绑定地址
    let addr: SocketAddr = format!("{}:{}", config.bind_address, config.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("🌐 Web interface ready at http://{}", addr);

    // 启动服务器并等待关闭信号
    let server_handle = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal());

    if let Err(e) = server_handle.await {
        error!("Server error: {}", e);
    }

    info!("👋 Hub shutdown complete");

    Ok(())
}

/// 初始化日志
fn init_logging(level: &str) -> anyhow::Result<()> {
    use tracing_subscriber::EnvFilter;

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    Ok(())
}

/// 加载配置
fn load_config(path: &str, args: &Args) -> anyhow::Result<HubConfig> {
    // 尝试从文件加载
    if std::path::Path::new(path).exists() {
        let content = std::fs::read_to_string(path)?;
        let config: HubConfig = toml::from_str(&content)?;
        info!("📄 Loaded config from: {}", path);
        return Ok(config);
    }

    // 使用命令行参数
    info!("📄 Using command-line config");
    Ok(HubConfig {
        hub_id: args.hub_id.clone(),
        bind_address: args.host.clone(),
        port: args.port,
        ..Default::default()
    })
}

/// 生成默认配置文件
fn generate_config(output: &str) -> anyhow::Result<()> {
    let config = HubConfig::default();
    let content = toml::to_string_pretty(&config)?;
    std::fs::write(output, content)?;
    println!("✅ Generated config file: {}", output);
    Ok(())
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
