//! # uHorse Hub
//!
//! 云端中枢二进制程序，负责管理节点、调度任务和路由消息。

use anyhow::Context;
use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, warn};

use uhorse_channel::DingTalkChannel;
use uhorse_config::{loader::create_default_loader, UHorseConfig};
use uhorse_core::Channel;
use uhorse_hub::{
    create_router,
    web::{reply_dingtalk_error, reply_task_result, submit_dingtalk_task},
    Hub, HubConfig, WebState,
};
use uhorse_llm::OpenAIClient;

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
    #[arg(long, default_value = "0.0.0.0")]
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

/// Hub 运行时配置
struct RuntimeConfig {
    app_config: UHorseConfig,
    hub_config: HubConfig,
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
    let runtime_config = load_config(&args.config, &args)?;

    // 初始化运行时依赖
    let dingtalk_channel = init_dingtalk_channel(&runtime_config.app_config).await?;
    let llm_client = init_llm_client(&runtime_config.app_config).await?;

    // 创建 Hub
    let (hub, mut task_result_rx) = Hub::new(runtime_config.hub_config.clone());
    let hub = Arc::new(hub);

    // 启动 Hub
    hub.start().await?;

    info!(
        "📡 Hub {} listening on {}:{}",
        runtime_config.hub_config.hub_id,
        runtime_config.hub_config.bind_address,
        runtime_config.hub_config.port
    );

    // 创建 Web 状态
    let web_state = WebState::new(hub.clone(), dingtalk_channel.clone(), llm_client);
    let shared_web_state = Arc::new(web_state.clone());
    let result_reply_state = shared_web_state.clone();

    tokio::spawn(async move {
        while let Some(task_result) = task_result_rx.recv().await {
            if let Err(error) = reply_task_result(result_reply_state.clone(), task_result).await {
                error!("Failed to reply task result to DingTalk: {}", error);
            }
        }
    });

    if let Some(channel) = dingtalk_channel {
        let stream_submit_state = shared_web_state.clone();
        let mut incoming_rx = channel.subscribe_incoming();
        tokio::spawn(async move {
            loop {
                match incoming_rx.recv().await {
                    Ok(inbound) => {
                        if let Err(error) = submit_dingtalk_task(&stream_submit_state, inbound.clone()).await {
                            error!("Failed to submit DingTalk stream task: {}", error);
                            if let Err(reply_error) = reply_dingtalk_error(
                                &stream_submit_state,
                                &inbound,
                                &error.to_string(),
                            )
                            .await
                            {
                                error!("Failed to reply DingTalk stream error: {}", reply_error);
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("DingTalk stream receiver lagged, skipped {} messages", skipped);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        warn!("DingTalk stream receiver closed");
                        break;
                    }
                }
            }
        });
    }

    // 创建路由
    let app = create_router(web_state);

    // 绑定地址
    let addr: SocketAddr = format!(
        "{}:{}",
        runtime_config.hub_config.bind_address, runtime_config.hub_config.port
    )
    .parse()?;
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
fn load_config(path: &str, args: &Args) -> anyhow::Result<RuntimeConfig> {
    let config_path = Path::new(path);

    if config_path.exists() {
        let content = std::fs::read_to_string(config_path)?;

        if looks_like_unified_config(&content) {
            let app_config = create_default_loader(config_path)
                .load()
                .with_context(|| format!("Failed to load unified config from: {}", path))?;

            info!("📄 Loaded unified config from: {}", path);

            let hub_config = HubConfig {
                hub_id: args.hub_id.clone(),
                bind_address: app_config.server.host.clone(),
                port: app_config.server.port,
                ..Default::default()
            };

            return Ok(RuntimeConfig {
                app_config,
                hub_config,
            });
        }

        let hub_config: HubConfig = toml::from_str(&content)?;
        let mut app_config = UHorseConfig::default();
        app_config.server.host = hub_config.bind_address.clone();
        app_config.server.port = hub_config.port;

        info!("📄 Loaded legacy hub config from: {}", path);

        return Ok(RuntimeConfig {
            app_config,
            hub_config,
        });
    }

    info!("📄 Using command-line config");

    let mut app_config = UHorseConfig::default();
    app_config.server.host = args.host.clone();
    app_config.server.port = args.port;

    let hub_config = HubConfig {
        hub_id: args.hub_id.clone(),
        bind_address: args.host.clone(),
        port: args.port,
        ..Default::default()
    };

    Ok(RuntimeConfig {
        app_config,
        hub_config,
    })
}

/// 判断是否为统一配置格式
fn looks_like_unified_config(content: &str) -> bool {
    [
        "[server]",
        "[database]",
        "[channels]",
        "[security]",
        "[logging]",
        "[observability]",
        "[scheduler]",
        "[tools]",
        "[llm]",
    ]
    .iter()
    .any(|marker| content.contains(marker))
}

/// 初始化 DingTalk 通道
async fn init_dingtalk_channel(
    config: &UHorseConfig,
) -> anyhow::Result<Option<Arc<DingTalkChannel>>> {
    if !config.channels.enabled.iter().any(|channel| channel == "dingtalk") {
        info!("📱 DingTalk channel is disabled in configuration");
        return Ok(None);
    }

    let dingtalk_config = config
        .channels
        .dingtalk
        .as_ref()
        .context("DingTalk channel is enabled but config is missing")?;

    info!("📱 Initializing DingTalk channel...");

    let mut channel = DingTalkChannel::new(
        dingtalk_config.app_key.clone(),
        dingtalk_config.app_secret.clone(),
        dingtalk_config.agent_id,
    );

    channel
        .start()
        .await
        .context("Failed to start DingTalk channel")?;

    info!("  ✓ DingTalk channel initialized");

    Ok(Some(Arc::new(channel)))
}

/// 初始化 LLM 客户端
async fn init_llm_client(config: &UHorseConfig) -> anyhow::Result<Option<Arc<OpenAIClient>>> {
    if !config.llm.enabled {
        info!("🤖 LLM is disabled in configuration");
        return Ok(None);
    }

    info!("🤖 Initializing LLM client...");
    info!("  Provider: {}", config.llm.provider);
    info!("  Model: {}", config.llm.model);

    let client = OpenAIClient::from_uhorse_config(config.llm.clone())
        .context("Failed to initialize LLM client")?;

    info!("  ✓ LLM client initialized successfully");

    Ok(Some(Arc::new(client)))
}

/// 生成默认配置文件
fn generate_config(output: &str) -> anyhow::Result<()> {
    let mut config = UHorseConfig::default();
    config.server.host = "0.0.0.0".to_string();
    config.server.port = 8765;

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
