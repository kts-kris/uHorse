//! # uHorse Node
//!
//! 本地执行节点二进制程序，负责接收云端中枢下发的命令并在本地执行。

use clap::{Parser, Subcommand};
use tokio::signal;
use tracing::{error, info, warn};

use uhorse_node::{ConnectionConfig, Node, NodeConfig, Workspace};

/// uHorse Node - 本地执行节点
#[derive(Parser, Debug)]
#[command(name = "uhorse-node")]
#[command(author = "uHorse Contributors")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "uHorse Local Node - Execute commands from Hub", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 配置文件路径
    #[arg(short, long, default_value = "node.toml")]
    config: String,

    /// 日志级别
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// Hub 地址
    #[arg(short, long, default_value = "ws://localhost:8765")]
    hub_url: String,

    /// 工作空间路径
    #[arg(short, long, default_value = ".")]
    workspace: String,

    /// 节点名称
    #[arg(long, default_value = "uHorse-Node")]
    name: String,
}

/// 子命令
#[derive(Subcommand, Debug)]
enum Commands {
    /// 生成默认配置文件
    Init {
        /// 输出路径
        #[arg(short, long, default_value = "node.toml")]
        output: String,
    },
    /// 检查工作空间权限
    Check {
        /// 工作空间路径
        #[arg(short, long, default_value = ".")]
        workspace: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // 处理子命令
    match args.command {
        Some(Commands::Init { output }) => {
            return generate_config(&output);
        }
        Some(Commands::Check { workspace }) => {
            return check_workspace(&workspace);
        }
        None => {}
    }

    // 初始化日志
    init_logging(&args.log_level)?;

    info!("🚀 uHorse Node v{} starting...", env!("CARGO_PKG_VERSION"));

    // 加载或使用默认配置
    let config = load_config(&args.config, &args)?;

    // 创建节点
    let mut node = Node::new(config)?;

    info!("📡 Connecting to Hub: {}", args.hub_url);
    info!("📁 Workspace: {}", args.workspace);

    // 启动节点
    if let Err(e) = node.start().await {
        error!("Failed to start node: {}", e);
        return Err(e.into());
    }

    // 等待关闭信号
    shutdown_signal().await;

    // 停止节点
    info!("🛑 Shutting down node...");
    if let Err(e) = node.stop().await {
        warn!("Error during shutdown: {}", e);
    }

    info!("👋 Node shutdown complete");

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
fn load_config(path: &str, args: &Args) -> anyhow::Result<NodeConfig> {
    // 尝试从文件加载
    if std::path::Path::new(path).exists() {
        let content = std::fs::read_to_string(path)?;
        let config: NodeConfig = toml::from_str(&content)?;
        info!("📄 Loaded config from: {}", path);
        return Ok(config);
    }

    // 使用命令行参数
    info!("📄 Using command-line config");
    Ok(NodeConfig {
        name: args.name.clone(),
        workspace_path: args.workspace.clone(),
        connection: ConnectionConfig {
            hub_url: args.hub_url.clone(),
            ..Default::default()
        },
        ..Default::default()
    })
}

/// 生成默认配置文件
fn generate_config(output: &str) -> anyhow::Result<()> {
    let config = NodeConfig::default();
    let content = toml::to_string_pretty(&config)?;
    std::fs::write(output, content)?;
    println!("✅ Generated config file: {}", output);
    Ok(())
}

/// 检查工作空间
fn check_workspace(workspace_path: &str) -> anyhow::Result<()> {
    println!("🔍 Checking workspace: {}", workspace_path);

    let _workspace = Workspace::new(workspace_path)?;

    println!("✅ Workspace validated successfully");
    println!("  Path: {}", workspace_path);
    println!("  Absolute: {:?}", std::fs::canonicalize(workspace_path)?);

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
