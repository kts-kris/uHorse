//! # 配置验证器
//!
//! 验证配置的有效性。

use super::UHorseConfig;
use anyhow::Result as AnyhowResult;
use std::path::Path;

/// 验证结果
#[derive(Debug)]
pub enum ValidationResult {
    Valid,
    Invalid { errors: Vec<String> },
}

impl ValidationResult {
    /// 是否有效
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// 获取错误列表
    pub fn errors(&self) -> &[String] {
        match self {
            Self::Valid => &[],
            Self::Invalid { errors } => errors,
        }
    }

    /// 合并多个验证结果
    pub fn merge(results: Vec<ValidationResult>) -> Self {
        let all_errors: Vec<_> = results
            .into_iter()
            .filter_map(|r| match r {
                ValidationResult::Invalid { errors } => Some(errors),
                _ => None,
            })
            .flatten()
            .collect();

        if all_errors.is_empty() {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid { errors: all_errors }
        }
    }
}

/// 配置验证器
pub trait ConfigValidator: Send + Sync {
    /// 验证配置
    fn validate(&self, config: &UHorseConfig) -> ValidationResult;
}

/// 服务器配置验证器
#[derive(Debug, Default)]
pub struct ServerValidator;

impl ConfigValidator for ServerValidator {
    fn validate(&self, config: &UHorseConfig) -> ValidationResult {
        let mut errors = Vec::new();

        // 验证端口范围
        if config.server.port < 1024 {
            errors.push("Server port < 1024 requires root privileges".to_string());
        }
        if config.server.port > 65535 {
            errors.push("Server port must be <= 65535".to_string());
        }

        // 验证超时设置
        if config.server.request_timeout == 0 {
            errors.push("Request timeout must be > 0".to_string());
        }
        if config.server.read_timeout == 0 {
            errors.push("Read timeout must be > 0".to_string());
        }
        if config.server.write_timeout == 0 {
            errors.push("Write timeout must be > 0".to_string());
        }

        // 验证 TLS 配置
        if let Some(tls) = &config.server.tls {
            if !tls.cert_path.exists() {
                errors.push(format!("TLS cert file not found: {:?}", tls.cert_path));
            }
            if !tls.key_path.exists() {
                errors.push(format!("TLS key file not found: {:?}", tls.key_path));
            }
            if let Some(ca_path) = &tls.ca_path {
                if !ca_path.exists() {
                    errors.push(format!("TLS CA file not found: {:?}", ca_path));
                }
            }
        }

        if errors.is_empty() {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid { errors }
        }
    }
}

/// 数据库配置验证器
#[derive(Debug, Default)]
pub struct DatabaseValidator;

impl ConfigValidator for DatabaseValidator {
    fn validate(&self, config: &UHorseConfig) -> ValidationResult {
        let mut errors = Vec::new();

        // 验证连接池大小
        if config.database.pool_size == 0 {
            errors.push("Database pool size must be > 0".to_string());
        }
        if config.database.pool_size > 100 {
            errors.push("Database pool size should be <= 100".to_string());
        }

        // 验证数据库路径的父目录存在
        let db_path = Path::new(&config.database.path);
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                errors.push(format!("Database parent directory does not exist: {:?}", parent));
            }
        }

        // 验证超时
        if config.database.conn_timeout == 0 {
            errors.push("Database connection timeout must be > 0".to_string());
        }

        if errors.is_empty() {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid { errors }
        }
    }
}

/// 安全配置验证器
#[derive(Debug, Default)]
pub struct SecurityValidator;

impl ConfigValidator for SecurityValidator {
    fn validate(&self, config: &UHorseConfig) -> ValidationResult {
        let mut errors = Vec::new();

        // 验证 JWT 密钥
        if let Some(secret) = &config.security.jwt_secret {
            if secret.len() < 32 {
                errors.push("JWT secret must be at least 32 characters".to_string());
            }
        } else {
            errors.push("JWT secret is required for production".to_string());
        }

        // 验证过期时间
        if config.security.token_expiry == 0 {
            errors.push("Token expiry must be > 0".to_string());
        }
        if config.security.refresh_token_expiry <= config.security.token_expiry {
            errors.push("Refresh token expiry must be > token expiry".to_string());
        }

        // 验证配对过期时间
        if config.security.pairing_expiry == 0 {
            errors.push("Pairing expiry must be > 0".to_string());
        }
        if config.security.pairing_expiry > 600 {
            errors.push("Pairing expiry should be <= 600 seconds (10 minutes)".to_string());
        }

        if errors.is_empty() {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid { errors }
        }
    }
}

/// 通道配置验证器
#[derive(Debug, Default)]
pub struct ChannelsValidator;

impl ConfigValidator for ChannelsValidator {
    fn validate(&self, config: &UHorseConfig) -> ValidationResult {
        let mut errors = Vec::new();

        // 验证启用的通道
        for channel in &config.channels.enabled {
            match channel.as_str() {
                "telegram" | "slack" | "discord" | "whatsapp" => {}
                _ => {
                    errors.push(format!("Unknown channel type: {}", channel));
                }
            }
        }

        // 验证 Telegram 配置
        if config.channels.enabled.contains(&"telegram".to_string()) {
            if let Some(telegram) = &config.channels.telegram {
                if telegram.bot_token.is_empty() {
                    errors.push("Telegram bot_token is required".to_string());
                }
                if !telegram.bot_token.starts_with("bot") || telegram.bot_token.len() < 45 {
                    errors.push("Invalid Telegram bot token format".to_string());
                }
            } else {
                errors.push("Telegram config is missing".to_string());
            }
        }

        // 验证 Slack 配置
        if config.channels.enabled.contains(&"slack".to_string()) {
            if let Some(slack) = &config.channels.slack {
                if slack.bot_token.is_empty() {
                    errors.push("Slack bot_token is required".to_string());
                }
                if slack.signing_secret.is_empty() {
                    errors.push("Slack signing_secret is required".to_string());
                }
            } else {
                errors.push("Slack config is missing".to_string());
            }
        }

        // 验证 Discord 配置
        if config.channels.enabled.contains(&"discord".to_string()) {
            if let Some(discord) = &config.channels.discord {
                if discord.bot_token.is_empty() {
                    errors.push("Discord bot_token is required".to_string());
                }
                if !discord.bot_token.starts_with("Bot ") {
                    errors.push("Discord bot_token must start with 'Bot '".to_string());
                }
            } else {
                errors.push("Discord config is missing".to_string());
            }
        }

        // 验证 WhatsApp 配置
        if config.channels.enabled.contains(&"whatsapp".to_string()) {
            if let Some(whatsapp) = &config.channels.whatsapp {
                if whatsapp.access_token.is_empty() {
                    errors.push("WhatsApp access_token is required".to_string());
                }
                if whatsapp.phone_number_id.is_empty() {
                    errors.push("WhatsApp phone_number_id is required".to_string());
                }
            } else {
                errors.push("WhatsApp config is missing".to_string());
            }
        }

        if errors.is_empty() {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid { errors }
        }
    }
}

/// 组合验证器
pub struct CompositeValidator {
    validators: Vec<Box<dyn ConfigValidator>>,
}

impl CompositeValidator {
    /// 创建新的组合验证器
    pub fn new() -> Self {
        Self {
            validators: Vec::new(),
        }
    }

    /// 添加验证器
    pub fn add(mut self, validator: Box<dyn ConfigValidator>) -> Self {
        self.validators.push(validator);
        self
    }

    /// 创建生产环境验证器
    pub fn production() -> Self {
        Self::new()
            .add(Box::new(ServerValidator::default()))
            .add(Box::new(DatabaseValidator::default()))
            .add(Box::new(SecurityValidator::default()))
            .add(Box::new(ChannelsValidator::default()))
    }

    /// 创建开发环境验证器（较宽松）
    pub fn development() -> Self {
        Self::new()
            .add(Box::new(ServerValidator::default()))
            .add(Box::new(DatabaseValidator::default()))
    }
}

impl Default for CompositeValidator {
    fn default() -> Self {
        Self::production()
    }
}

impl ConfigValidator for CompositeValidator {
    fn validate(&self, config: &UHorseConfig) -> ValidationResult {
        let results: Vec<_> = self.validators
            .iter()
            .map(|v| v.validate(config))
            .collect();

        ValidationResult::merge(results)
    }
}

/// 验证配置
pub fn validate_config(config: &UHorseConfig, production: bool) -> AnyhowResult<()> {
    let validator = if production {
        CompositeValidator::production()
    } else {
        CompositeValidator::development()
    };

    match validator.validate(config) {
        ValidationResult::Valid => Ok(()),
        ValidationResult::Invalid { errors } => {
            let error_msg = format!("Configuration validation failed:\n{}", errors.join("\n"));
            Err(anyhow::anyhow!(error_msg))
        }
    }
}
