//! # uHorse 配置管理
//!
//! 提供配置加载、验证和热加载功能。

pub mod distributed;
pub mod hot_reload;
pub mod loader;
pub mod source;
pub mod validator;
pub mod versioning;
pub mod wizard;

pub use loader::{ConfigLoader, ConfigWatch};
pub use source::{ConfigSource, ConfigValue, MergeStrategy};
pub use validator::{ConfigValidator, ValidationResult};
pub use wizard::{CliWizard, ConfigWizard};

// Re-exports for distributed config
pub use distributed::{
    ConfigBackend, ConfigWatchEvent, ConfigWatchStream, DistributedConfigClient,
    DistributedConfigOptions, InMemoryConfigBackend,
};
pub use hot_reload::{
    ConfigChangeEvent, ConfigReloader, HotReloadBuilder, HotReloadManager, ReloadableConfig,
};
pub use versioning::{ConfigDiff, ConfigHistory, ConfigRollback, ConfigVersion, DiffLine};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// uHorse 主配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UHorseConfig {
    /// 服务器配置
    pub server: ServerConfig,
    /// 数据库配置
    pub database: DatabaseConfig,
    /// 通道配置
    pub channels: ChannelsConfig,
    /// 安全配置
    pub security: SecurityConfig,
    /// 日志配置
    pub logging: LoggingConfig,
    /// 可观测性配置
    pub observability: ObservabilityConfig,
    /// 调度器配置
    pub scheduler: SchedulerConfig,
    /// 工具配置
    pub tools: ToolsConfig,
    /// LLM 配置
    #[serde(default)]
    pub llm: LLMConfig,
}

#[allow(clippy::derivable_impls)]
impl Default for UHorseConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            channels: ChannelsConfig::default(),
            security: SecurityConfig::default(),
            logging: LoggingConfig::default(),
            observability: ObservabilityConfig::default(),
            scheduler: SchedulerConfig::default(),
            tools: ToolsConfig::default(),
            llm: LLMConfig::default(),
        }
    }
}

/// 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 监听地址
    #[serde(default = "default_server_host")]
    pub host: String,
    /// 监听端口
    #[serde(default = "default_server_port")]
    pub port: u16,
    /// 最大连接数
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
    /// 请求超时（秒)
    #[serde(default = "default_request_timeout")]
    pub request_timeout: u64,
    /// 读取超时（秒)
    #[serde(default = "default_read_timeout")]
    pub read_timeout: u64,
    /// 写入超时（秒)
    #[serde(default = "default_write_timeout")]
    pub write_timeout: u64,
    /// TLS 配置
    #[serde(default)]
    pub tls: Option<TlsConfig>,
    /// 健康检查配置
    #[serde(default)]
    pub health: HealthConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_server_host(),
            port: default_server_port(),
            max_connections: default_max_connections(),
            request_timeout: default_request_timeout(),
            read_timeout: default_read_timeout(),
            write_timeout: default_write_timeout(),
            tls: None,
            health: HealthConfig::default(),
        }
    }
}

fn default_server_host() -> String {
    "0.0.0.0".to_string()
}
fn default_server_port() -> u16 {
    8765
}
fn default_max_connections() -> usize {
    1000
}
fn default_request_timeout() -> u64 {
    30
}
fn default_read_timeout() -> u64 {
    10
}
fn default_write_timeout() -> u64 {
    10
}

/// TLS 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// 证书文件路径
    pub cert_path: PathBuf,
    /// 私钥文件路径
    pub key_path: PathBuf,
    /// CA 证书路径（可选）
    pub ca_path: Option<PathBuf>,
}

/// 健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HealthConfig {
    /// 是否启用健康检查端点
    pub enabled: bool,
    /// 健康检查路径
    pub path: String,
    /// 详细信息（暴露内部状态）
    pub verbose: bool,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "/api/health".to_string(),
            verbose: false,
        }
    }
}

/// 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// 数据库路径
    #[serde(default = "default_db_path")]
    pub path: String,
    /// 连接池大小
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,
    /// 连接超时（秒)
    #[serde(default = "default_conn_timeout")]
    pub conn_timeout: u64,
    /// 启用 WAL 模式
    #[serde(default = "default_wal_enabled")]
    pub wal_enabled: bool,
    /// 启用外键约束
    #[serde(default = "default_fk_enabled")]
    pub fk_enabled: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
            pool_size: default_pool_size(),
            conn_timeout: default_conn_timeout(),
            wal_enabled: default_wal_enabled(),
            fk_enabled: default_fk_enabled(),
        }
    }
}

fn default_db_path() -> String {
    "./data/uhorse.db".to_string()
}
fn default_pool_size() -> usize {
    10
}
fn default_conn_timeout() -> u64 {
    30
}
fn default_wal_enabled() -> bool {
    true
}
fn default_fk_enabled() -> bool {
    true
}

/// 通道配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelsConfig {
    /// 启用的通道列表
    pub enabled: Vec<String>,
    /// Telegram 配置
    pub telegram: Option<TelegramConfig>,
    /// DingTalk 配置
    pub dingtalk: Option<DingTalkConfig>,
    /// Slack 配置
    pub slack: Option<SlackConfig>,
    /// Discord 配置
    pub discord: Option<DiscordConfig>,
    /// WhatsApp 配置
    pub whatsapp: Option<WhatsAppConfig>,
}

#[allow(clippy::derivable_impls)]
impl Default for ChannelsConfig {
    fn default() -> Self {
        Self {
            enabled: vec![],
            telegram: None,
            dingtalk: None,
            slack: None,
            discord: None,
            whatsapp: None,
        }
    }
}

/// Telegram 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    /// Bot Token
    pub bot_token: String,
    /// Webhook URL（可选）
    pub webhook_url: Option<String>,
    /// 最大连接数
    #[serde(default = "default_telegram_max_connections")]
    pub max_connections: usize,
}

fn default_telegram_max_connections() -> usize {
    100
}

/// DingTalk 通知绑定
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DingTalkNotificationBinding {
    /// 节点 ID
    pub node_id: String,
    /// 接收通知的 DingTalk 用户 ID
    pub user_id: String,
}

/// DingTalk 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkConfig {
    /// App Key
    pub app_key: String,
    /// App Secret
    pub app_secret: String,
    /// Agent ID
    pub agent_id: u64,
    /// 节点通知绑定
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notification_bindings: Vec<DingTalkNotificationBinding>,
}

/// Slack 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    /// Bot Token
    pub bot_token: String,
    /// Signing Secret（用于 webhook 验证）
    pub signing_secret: String,
    /// App Token（用于 Socket Mode）
    pub app_token: Option<String>,
}

/// Discord 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    /// Bot Token
    pub bot_token: String,
    /// Application ID
    pub application_id: String,
    /// 最大连接数
    #[serde(default = "default_discord_shards")]
    pub max_shards: u32,
}

fn default_discord_shards() -> u32 {
    1
}

/// WhatsApp 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppConfig {
    /// Access Token
    pub access_token: String,
    /// Phone Number ID
    pub phone_number_id: String,
    /// Business Account ID
    pub business_account_id: String,
    /// Webhook Verify Token
    pub webhook_verify_token: String,
}

/// 安全配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    /// JWT 密钥
    pub jwt_secret: Option<String>,
    /// 令牌过期时间（秒）
    #[serde(default = "default_token_expiry")]
    pub token_expiry: u64,
    /// 刷新令牌过期时间（秒）
    #[serde(default = "default_refresh_expiry")]
    pub refresh_token_expiry: u64,
    /// 设备配对码过期时间（秒）
    #[serde(default = "default_pairing_expiry")]
    pub pairing_expiry: u64,
    /// 启用审批流程
    #[serde(default = "default_approval_enabled")]
    pub approval_enabled: bool,
    /// 启用设备配对
    #[serde(default = "default_pairing_enabled")]
    pub pairing_enabled: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            jwt_secret: None,
            token_expiry: default_token_expiry(),
            refresh_token_expiry: default_refresh_expiry(),
            pairing_expiry: default_pairing_expiry(),
            approval_enabled: default_approval_enabled(),
            pairing_enabled: default_pairing_enabled(),
        }
    }
}

fn default_token_expiry() -> u64 {
    86400
} // 24 小时
fn default_refresh_expiry() -> u64 {
    2592000
} // 30 天
fn default_pairing_expiry() -> u64 {
    300
} // 5 分钟
fn default_approval_enabled() -> bool {
    true
}
fn default_pairing_enabled() -> bool {
    true
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// 日志级别
    pub level: String,
    /// 日志格式
    pub format: LogFormat,
    /// 日志输出
    #[serde(default)]
    pub output: LogOutput,
    /// 是否启用颜色
    pub ansi: bool,
    /// 是否显示文件名
    pub file: bool,
    /// 是否显示行号
    pub line: bool,
    /// 是否显示目标
    pub target: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: LogFormat::Full,
            output: LogOutput::default(),
            ansi: true,
            file: true,
            line: true,
            target: true,
        }
    }
}

/// 日志格式
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// 完整格式
    Full,
    /// 紧凑格式
    Compact,
    /// JSON 格式
    Json,
    /// Pretty 格式
    Pretty,
}

/// 日志输出
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogOutput {
    /// 标准输出
    Stdout,
    /// 标准错误
    Stderr,
    /// 文件输出
    File { path: PathBuf, rotate: bool },
    /// 同时输出到文件和终端
    Both { file: PathBuf, rotate: bool },
}

#[allow(clippy::derivable_impls)]
impl Default for LogOutput {
    fn default() -> Self {
        Self::Stdout
    }
}

/// 可观测性配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ObservabilityConfig {
    /// 服务名称
    pub service_name: String,
    /// 是否启用 Tracing
    pub tracing_enabled: bool,
    /// 是否启用 Metrics
    pub metrics_enabled: bool,
    /// OTLP 端点
    pub otlp_endpoint: Option<String>,
    /// Metrics 导出端口
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            service_name: "uhorse-hub".to_string(),
            tracing_enabled: true,
            metrics_enabled: true,
            otlp_endpoint: None,
            metrics_port: default_metrics_port(),
        }
    }
}

fn default_metrics_port() -> u16 {
    9090
}

/// 调度器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SchedulerConfig {
    /// 是否启用
    pub enabled: bool,
    /// 工作线程数
    #[serde(default = "default_scheduler_threads")]
    pub threads: usize,
    /// 最大并发任务数
    #[serde(default = "default_max_concurrent_jobs")]
    pub max_concurrent_jobs: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threads: default_scheduler_threads(),
            max_concurrent_jobs: default_max_concurrent_jobs(),
        }
    }
}

fn default_scheduler_threads() -> usize {
    2
}
fn default_max_concurrent_jobs() -> usize {
    100
}

/// 工具配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolsConfig {
    /// 是否启用沙箱
    pub sandbox_enabled: bool,
    /// 沙箱超时（秒)
    #[serde(default = "default_sandbox_timeout")]
    pub sandbox_timeout: u64,
    /// 最大内存（MB）
    #[serde(default = "default_sandbox_max_memory")]
    pub sandbox_max_memory: usize,
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            sandbox_enabled: true,
            sandbox_timeout: default_sandbox_timeout(),
            sandbox_max_memory: default_sandbox_max_memory(),
        }
    }
}

fn default_sandbox_timeout() -> u64 {
    30
}
fn default_sandbox_max_memory() -> usize {
    512
}

/// LLM 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LLMConfig {
    /// 是否启用 LLM
    pub enabled: bool,

    /// 服务商
    pub provider: String,

    /// API 密钥
    pub api_key: String,

    /// API 基础 URL
    #[serde(default = "default_llm_base_url")]
    pub base_url: String,

    /// 模型名称
    pub model: String,

    /// 温度 (0.0 - 2.0)
    #[serde(default = "default_llm_temperature")]
    pub temperature: f32,

    /// 最大 tokens
    #[serde(default = "default_llm_max_tokens")]
    pub max_tokens: usize,

    /// 系统提示词
    #[serde(default = "default_llm_system_prompt")]
    pub system_prompt: String,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "openai".to_string(),
            api_key: String::new(),
            base_url: default_llm_base_url(),
            model: "gpt-3.5-turbo".to_string(),
            temperature: default_llm_temperature(),
            max_tokens: default_llm_max_tokens(),
            system_prompt: default_llm_system_prompt(),
        }
    }
}

fn default_llm_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}
fn default_llm_temperature() -> f32 {
    0.7
}
fn default_llm_max_tokens() -> usize {
    2000
}
fn default_llm_system_prompt() -> String {
    "You are a helpful AI assistant for uHorse, a multi-channel AI gateway.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dingtalk_notification_bindings_default_to_empty() {
        let config: UHorseConfig = toml::from_str(
            r#"
[channels]
enabled = ["dingtalk"]

[channels.dingtalk]
app_key = "key"
app_secret = "secret"
agent_id = 1
"#,
        )
        .unwrap();

        let dingtalk = config.channels.dingtalk.unwrap();
        assert!(dingtalk.notification_bindings.is_empty());
    }

    #[test]
    fn test_dingtalk_notification_bindings_deserialize() {
        let config: UHorseConfig = toml::from_str(
            r#"
[channels]
enabled = ["dingtalk"]

[channels.dingtalk]
app_key = "key"
app_secret = "secret"
agent_id = 1

[[channels.dingtalk.notification_bindings]]
node_id = "node-desktop-test"
user_id = "manager-1"
"#,
        )
        .unwrap();

        let dingtalk = config.channels.dingtalk.unwrap();
        assert_eq!(
            dingtalk.notification_bindings,
            vec![DingTalkNotificationBinding {
                node_id: "node-desktop-test".to_string(),
                user_id: "manager-1".to_string(),
            }]
        );
    }

    #[test]
    fn test_default_hub_runtime_values_align_with_mainline() {
        let config = UHorseConfig::default();

        assert_eq!(config.server.port, 8765);
        assert_eq!(config.server.health.path, "/api/health");
        assert_eq!(config.observability.service_name, "uhorse-hub");
    }
}
