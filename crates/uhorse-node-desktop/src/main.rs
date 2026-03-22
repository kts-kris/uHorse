//! # uHorse Node Desktop
//!
//! Node 桌面客户端 MVP 入口，当前提供桌面模式占位与运行时健康检查。

use clap::{Parser, Subcommand};
use tracing::info;
use uhorse_node_runtime::{NodeConfig, Workspace};

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
}

/// 桌面子命令
#[derive(Subcommand, Debug)]
enum Commands {
    /// 检查桌面运行时基础依赖
    Doctor,
    /// 输出桌面客户端默认配置
    PrintConfig,
}

fn init_logging() {
    use tracing_subscriber::EnvFilter;

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}

fn main() -> anyhow::Result<()> {
    init_logging();

    let args = Args::parse();
    match args.command.unwrap_or(Commands::Doctor) {
        Commands::Doctor => doctor(&args.workspace),
        Commands::PrintConfig => print_config(&args.workspace),
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
