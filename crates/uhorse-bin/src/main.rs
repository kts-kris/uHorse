//! # uHorse AI Gateway
//!
//! 多渠道 AI 网关框架主程序。

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uhorse_core::Channel;
use uhorse_llm::{ChatMessage, LLMClient, OpenAIClient};

/// uHorse AI Gateway
#[derive(Parser, Debug)]
#[command(name = "uhorse")]
#[command(author = "uHorse Contributors")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Multi-channel AI Gateway Framework", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

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

/// 子命令
#[derive(Subcommand, Debug)]
enum Commands {
    /// 启动交互式配置向导
    Wizard {
        /// 项目目录（默认当前目录）
        #[arg(short, long, default_value = ".")]
        dir: String,
    },
    /// 运行服务器
    Run,
}

/// 共享状态
struct AppState {
    telegram_channel: Arc<RwLock<Option<uhorse_channel::TelegramChannel>>>,
    llm_client: Arc<RwLock<Option<OpenAIClient>>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // 处理子命令
    match args.command {
        Some(Commands::Wizard { dir }) => {
            return run_wizard(dir);
        }
        Some(Commands::Run) | None => {
            // 默认运行服务器
        }
    }

    // 初始化日志
    init_logging(&args.log_level)?;

    info!(
        "🦀 uHorse AI Gateway v{} starting...",
        env!("CARGO_PKG_VERSION")
    );

    // 加载配置
    info!("📄 Loading config from: {}", args.config);
    let file_config = load_config(&args.config)?;

    // 初始化通道
    let telegram_channel = init_channels(&file_config).await?;

    // 初始化 LLM 客户端
    let llm_client = init_llm_client(&file_config).await?;

    // 创建共享状态
    let state = Arc::new(AppState {
        telegram_channel: Arc::new(RwLock::new(telegram_channel)),
        llm_client: Arc::new(RwLock::new(llm_client)),
    });

    // 启动 Telegram polling 任务
    let telegram_polling_handle = start_telegram_polling(state.clone()).await?;

    // 构建路由
    let app = Router::new()
        .route("/health/live", get(health_live))
        .route("/health/ready", get(health_ready))
        .route("/metrics", get(metrics))
        .route("/api/v1/channels/telegram/webhook", post(telegram_webhook))
        .route(
            "/api/v1/channels/telegram/webhook",
            get(telegram_webhook_verify),
        )
        .with_state(state.clone());

    // 启动服务器
    let addr = format!("{}:{}", file_config.server.host, file_config.server.port);
    let socket_addr: SocketAddr = addr.parse()?;
    info!("🚀 Starting server on http://{}", socket_addr);

    let listener = tokio::net::TcpListener::bind(socket_addr).await?;

    info!("🚀 Server ready to accept connections");
    info!(
        "📱 Telegram channel: {}",
        if file_config
            .channels
            .enabled
            .contains(&"telegram".to_string())
        {
            "enabled"
        } else {
            "disabled"
        }
    );
    info!("💡 Tip: Send a message to your bot to test");

    // 启动服务器并等待关闭信号
    let server_handle = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal());

    // 等待服务器完成
    if let Err(e) = server_handle.await {
        error!("Server error: {}", e);
    }

    // 停止 polling 任务
    if let Some(handle) = telegram_polling_handle {
        handle.abort();
    }

    info!("👋 Shutting down gracefully...");

    Ok(())
}

/// 初始化通道
async fn init_channels(config: &Config) -> anyhow::Result<Option<uhorse_channel::TelegramChannel>> {
    info!("📱 Initializing channels...");

    let mut telegram_channel = None;

    // 初始化 Telegram
    if config.channels.enabled.contains(&"telegram".to_string()) {
        if let Some(telegram_config) = &config.channels.telegram {
            info!("  → Initializing Telegram channel...");

            use uhorse_channel::TelegramChannel;
            let mut channel = TelegramChannel::new(telegram_config.bot_token.clone());

            // 启动通道（验证 API 连接）
            if let Err(e) = channel.start().await {
                warn!("Failed to start Telegram channel: {}", e);
                warn!("Telegram features will be disabled");
            } else {
                info!("  ✓ Telegram channel initialized");
                telegram_channel = Some(channel);
            }
        }
    }

    if telegram_channel.is_none() {
        info!("  No channels enabled");
    }

    Ok(telegram_channel)
}

/// 初始化 LLM 客户端
async fn init_llm_client(config: &Config) -> anyhow::Result<Option<OpenAIClient>> {
    if !config.llm.enabled {
        info!("🤖 LLM is disabled in configuration");
        return Ok(None);
    }

    info!("🤖 Initializing LLM client...");
    info!("  Provider: {}", config.llm.provider);
    info!("  Model: {}", config.llm.model);

    // 构建完整的 LLM 配置
    let full_llm_config = uhorse_config::LLMConfig {
        enabled: true,
        provider: config.llm.provider.clone(),
        api_key: config.llm.api_key.clone(),
        base_url: config.llm.base_url.clone(),
        model: config.llm.model.clone(),
        temperature: config.llm.temperature,
        max_tokens: config.llm.max_tokens,
        system_prompt: "You are a helpful AI assistant for uHorse, a multi-channel AI gateway."
            .to_string(),
    };

    match OpenAIClient::from_uhorse_config(full_llm_config) {
        Ok(client) => {
            info!("  ✓ LLM client initialized successfully");
            Ok(Some(client))
        }
        Err(e) => {
            warn!("  ✗ Failed to initialize LLM client: {}", e);
            warn!("  LLM features will be disabled");
            Ok(None)
        }
    }
}

/// 启动 Telegram polling
async fn start_telegram_polling(
    state: Arc<AppState>,
) -> anyhow::Result<Option<tokio::task::JoinHandle<()>>> {
    let channel_guard = state.telegram_channel.read().await;

    if channel_guard.is_none() {
        drop(channel_guard);
        return Ok(None);
    }

    let channel = channel_guard.as_ref().unwrap().clone();
    drop(channel_guard);

    info!("🔄 Starting Telegram polling...");

    let handle = tokio::spawn(async move {
        let mut offset = 0i32;

        loop {
            // 获取最新的通道引用
            let channel_guard = state.telegram_channel.read().await;
            let channel_opt = channel_guard.as_ref().cloned();
            drop(channel_guard);

            // 获取 LLM 客户端
            let llm_guard = state.llm_client.read().await;
            let llm_opt = llm_guard.as_ref();

            let result = if let Some(channel) = channel_opt {
                poll_telegram_updates(&channel, &mut offset, llm_opt).await
            } else {
                Ok(())
            };

            drop(llm_guard);

            if let Err(e) = result {
                warn!("Telegram polling error: {}", e);
            }

            // 每 3 秒轮询一次
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });

    Ok(Some(handle))
}

/// 轮询 Telegram 更新
async fn poll_telegram_updates(
    channel: &uhorse_channel::TelegramChannel,
    offset: &mut i32,
    llm_client: Option<&OpenAIClient>,
) -> anyhow::Result<()> {
    use reqwest::Client;

    let client = Client::new();
    let url = format!(
        "https://api.telegram.org/bot{}/getUpdates",
        channel.bot_token()
    );

    let response = client
        .get(&url)
        .query(&[
            ("offset", offset.to_string()),
            ("timeout", "10".to_string()),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Telegram API error: {}", response.status()));
    }

    let json: Value = response.json().await?;

    if let Some(result) = json.get("result").and_then(|v| v.as_array()) {
        for update in result {
            // 处理更新
            if let Err(e) = handle_telegram_update(channel, update, llm_client).await {
                error!("Error handling Telegram update: {}", e);
            }

            // 更新 offset
            if let Some(update_id) = update.get("update_id").and_then(|v| v.as_i64()) {
                *offset = (update_id + 1) as i32;
            }
        }
    }

    Ok(())
}

/// 处理 Telegram 更新
async fn handle_telegram_update(
    channel: &uhorse_channel::TelegramChannel,
    update: &Value,
    llm_client: Option<&OpenAIClient>,
) -> anyhow::Result<()> {
    let update_json = serde_json::to_string(update)?;

    if let Some((session, message)) = channel.handle_update_raw(&update_json).await? {
        info!(
            "📨 Received message from {}: {:?}",
            session.channel_user_id, message.content
        );

        // 处理消息并发送回复
        match &message.content {
            uhorse_core::MessageContent::Text(text) => {
                let reply = if let Some(client) = llm_client {
                    // 使用 LLM 处理消息
                    info!("🤖 Processing message with LLM...");
                    match process_with_llm(client, text).await {
                        Ok(llm_reply) => llm_reply,
                        Err(e) => {
                            error!("LLM processing failed: {}", e);
                            format!("抱歉，处理消息时出错: {}", e)
                        }
                    }
                } else {
                    // 无 LLM 时的简单回复
                    format!("收到你的消息: {}", text)
                };

                if let Err(e) = channel
                    .send_message(
                        &session.channel_user_id,
                        &uhorse_core::MessageContent::Text(reply.clone()),
                    )
                    .await
                {
                    error!("Failed to send reply: {}", e);
                } else {
                    info!("✓ Reply sent: {}", reply);
                }
            }
            _ => {
                let reply = "收到你的消息！";
                if let Err(e) = channel
                    .send_message(
                        &session.channel_user_id,
                        &uhorse_core::MessageContent::Text(reply.to_string()),
                    )
                    .await
                {
                    error!("Failed to send reply: {}", e);
                } else {
                    info!("✓ Reply sent successfully");
                }
            }
        }
    }

    Ok(())
}

/// 使用 LLM 处理消息
async fn process_with_llm(client: &OpenAIClient, text: &str) -> anyhow::Result<String> {
    let messages = vec![
        ChatMessage::system("You are a helpful AI assistant for uHorse, a multi-channel AI gateway. Be concise and friendly.".to_string()),
        ChatMessage::user(text.to_string()),
    ];

    client.chat_completion(messages).await
}

/// Telegram webhook 端点
async fn telegram_webhook(
    State(state): State<Arc<AppState>>,
    payload: String,
) -> Result<Json<Value>, StatusCode> {
    info!("📨 Received Telegram webhook");

    let channel_guard = state.telegram_channel.read().await;
    let llm_guard = state.llm_client.read().await;
    if let Some(channel) = channel_guard.as_ref() {
        let llm_opt = llm_guard.as_ref();
        // 处理 webhook payload
        if let Err(e) = handle_telegram_update(
            channel,
            &serde_json::from_str(&payload).unwrap_or(Value::Null),
            llm_opt,
        )
        .await
        {
            error!("Webhook error: {}", e);
            drop(llm_guard);
            drop(channel_guard);
            return Ok(Json(
                serde_json::json!({"status": "error", "message": e.to_string()}),
            ));
        }
    }
    drop(llm_guard);
    drop(channel_guard);

    Ok(Json(serde_json::json!({"status": "ok"})))
}

/// Telegram webhook 验证端点
async fn telegram_webhook_verify() -> &'static str {
    "Telegram webhook endpoint is ready"
}

/// 运行配置向导
fn run_wizard(dir: String) -> anyhow::Result<()> {
    use uhorse_config::CliWizard;

    let mut wizard = CliWizard::new(dir);
    wizard.run()?;
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
    #[serde(default)]
    llm: LLMConfig,
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

fn default_host() -> String {
    "0.0.0.0".to_string()
}
fn default_port() -> u16 {
    8080
}
fn default_max_connections() -> usize {
    1000
}

#[derive(Debug, serde::Deserialize, Default)]
struct ChannelsConfig {
    #[serde(default)]
    enabled: Vec<String>,
    #[serde(default)]
    telegram: Option<TelegramConfig>,
}

#[derive(Debug, serde::Deserialize)]
struct TelegramConfig {
    bot_token: String,
}

#[derive(Debug, serde::Deserialize, Default)]
struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    path: String,
}

fn default_db_path() -> String {
    "./data/uhorse.db".to_string()
}

#[derive(Debug, serde::Deserialize, Default)]
struct SecurityConfig {
    #[serde(default)]
    jwt_secret: Option<String>,
    #[serde(default = "default_token_expiry")]
    token_expiry: u64,
}

fn default_token_expiry() -> u64 {
    86400
}

#[derive(Debug, serde::Deserialize, Default)]
struct LLMConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_llm_provider")]
    provider: String,
    #[serde(default)]
    api_key: String,
    #[serde(default = "default_llm_base_url")]
    base_url: String,
    #[serde(default = "default_llm_model")]
    model: String,
    #[serde(default = "default_llm_temperature")]
    temperature: f32,
    #[serde(default = "default_llm_max_tokens")]
    max_tokens: usize,
}

fn default_llm_provider() -> String {
    "openai".to_string()
}
fn default_llm_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}
fn default_llm_model() -> String {
    "gpt-3.5-turbo".to_string()
}
fn default_llm_temperature() -> f32 {
    0.7
}
fn default_llm_max_tokens() -> usize {
    2000
}

/// 健康检查响应
#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

/// 存活性检查
async fn health_live() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// 就绪性检查
async fn health_ready() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ready".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// 指标端点
async fn metrics() -> &'static str {
    "# uhorse metrics (placeholder)
# HELP uhorse_up uHorse is running
# TYPE uhorse_up gauge
uhorse_up 1
"
}

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
