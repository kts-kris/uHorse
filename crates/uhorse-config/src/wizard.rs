//! # uHorse 配置向导
//!
//! 交互式配置向导工具

use std::io::{self, Write};
use std::fs;
use std::path::Path;

/// 配置向导
pub struct ConfigWizard {
    project_dir: String,
    config: ConfigData,
}

#[derive(Debug, Default)]
struct ConfigData {
    server: ServerConfig,
    channels: ChannelsConfig,
    database: DatabaseConfig,
    llm: LLMConfig,
    security: SecurityConfig,
}

#[derive(Debug, Default)]
struct ServerConfig {
    host: String,
    port: u16,
}

#[derive(Debug, Default)]
struct ChannelsConfig {
    telegram: Option<ChannelConfig>,
    slack: Option<ChannelConfig>,
    discord: Option<ChannelConfig>,
    whatsapp: Option<ChannelConfig>,
    dingtalk: Option<ChannelConfig>,
    feishu: Option<ChannelConfig>,
    wework: Option<ChannelConfig>,
}

#[derive(Debug, Default)]
struct ChannelConfig {
    enabled: bool,
    bot_token: Option<String>,
    extra: std::collections::HashMap<String, String>,
}

#[derive(Debug, Default)]
struct DatabaseConfig {
    db_type: String,
    path: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Default)]
struct SecurityConfig {
    jwt_secret: String,
    token_expiry: u64,
}

#[derive(Debug, Default)]
struct LLMConfig {
    enabled: bool,
    provider: String,
    api_key: String,
    base_url: String,
    model: String,
    temperature: f32,
    max_tokens: usize,
}

impl ConfigWizard {
    pub fn new(project_dir: String) -> Self {
        Self {
            project_dir,
            config: ConfigData::default(),
        }
    }

    /// 启动向导
    pub fn start(&mut self) -> anyhow::Result<()> {
        self.print_welcome();
        self.configure_server()?;
        self.configure_database()?;
        self.configure_channels()?;
        self.configure_llm()?;
        self.configure_security()?;
        self.validate_config()?;
        self.save_config()?;
        self.print_next_steps();

        Ok(())
    }

    /// 打印欢迎信息
    fn print_welcome(&self) {
        println!();
        println!("╔════════════════════════════════════════════════╗");
        println!("║                                                ║");
        println!("║       🦄 uHorse 配置向导                         ║");
        println!("║       Interactive Configuration Wizard          ║");
        println!("║                                                ║");
        println!("╚════════════════════════════════════════════════╝");
        println!();
        println!("这个向导将帮助您配置 uHorse 多渠道 AI 网关。");
        println!("您将需要提供以下信息：");
        println!("  • 服务器地址和端口");
        println!("  • 数据库配置");
        println!("  • 通道凭证 (Telegram, Slack, Discord, WhatsApp)");
        println!("  • 大语言模型配置 (可选)");
        println!("  • 安全设置");
        println!();
        self.press_enter_to_continue();
    }

    /// 配置服务器
    fn configure_server(&mut self) -> anyhow::Result<()> {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  📡 服务器配置");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();

        // 默认值
        let default_host = "127.0.0.1".to_string();
        let default_port = "8080".to_string();

        // 询问主机地址
        self.config.server.host = self.prompt_input(
            &format!("监听地址 [{}]: ", default_host),
            default_host.clone(),
        )?;

        // 询问端口
        let port_str = self.prompt_input(
            &format!("监听端口 [{}]: ", default_port),
            default_port,
        )?;
        self.config.server.port = port_str.parse::<u16>()
            .map_err(|_| anyhow::anyhow!("无效的端口号"))?;

        // 显示配置
        println!();
        println!("服务器配置:");
        println!("  监听地址: {}", self.config.server.host);
        println!("  监听端口: {}", self.config.server.port);
        println!();

        self.confirm_or_edit_server()?;

        Ok(())
    }

    /// 确认或编辑服务器配置
    fn confirm_or_edit_server(&mut self) -> anyhow::Result<()> {
        loop {
            let choice = self.prompt_choice("是否正确? ", &["确认", "重新配置"])?;

            match choice.as_str() {
                "重新配置" => {
                    return self.configure_server();
                }
                _ => return Ok(()),
            }
        }
    }

    /// 配置数据库
    fn configure_database(&mut self) -> anyhow::Result<()> {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  💾 数据库配置");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();

        let db_type = self.prompt_choice(
            "选择数据库类型: ",
            &["SQLite (推荐)", "PostgreSQL"],
        )?;

        match db_type.as_str() {
            "SQLite (推荐)" => {
                self.config.database.db_type = "sqlite".to_string();
                let default_path = "./data/uhorse.db".to_string();
                self.config.database.path = Some(self.prompt_input(
                    &format!("数据库文件路径 [{}]: ", default_path),
                    default_path.clone(),
                )?);
                self.config.database.url = None;
            }
            "PostgreSQL" => {
                self.config.database.db_type = "postgresql".to_string();
                let default_url = "postgresql://uhorse:password@localhost:5432/uhorse".to_string();
                self.config.database.url = Some(self.prompt_input(
                    &format!("连接 URL [{}]: ", default_url),
                    default_url.clone(),
                )?);
                self.config.database.path = None;
            }
            _ => {
                return Err(anyhow::anyhow!("无效的数据库类型选择"));
            }
        }

        println!();
        println!("数据库配置:");
        if self.config.database.db_type == "sqlite" {
            println!("  类型: SQLite");
            println!("  路径: {}", self.config.database.path.as_ref().unwrap());
        } else {
            println!("  类型: PostgreSQL");
            println!("  URL: {}", self.config.database.url.as_ref().unwrap());
        }
        println!();

        self.confirm_or_edit_database()?;

        Ok(())
    }

    /// 确认或编辑数据库配置
    fn confirm_or_edit_database(&mut self) -> anyhow::Result<()> {
        loop {
            let choice = self.prompt_choice("是否正确? ", &["确认", "重新配置"])?;

            match choice.as_str() {
                "重新配置" => {
                    return self.configure_database();
                }
                _ => return Ok(()),
            }
        }
    }

    /// 配置 LLM
    fn configure_llm(&mut self) -> anyhow::Result<()> {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  🤖 大语言模型配置");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();

        let enable_llm = self.prompt_choice(
            "是否启用大语言模型功能? ",
            &["启用", "跳过"],
        )?;

        if enable_llm != "启用" {
            self.config.llm.enabled = false;
            println!("跳过 LLM 配置");
            return Ok(());
        }

        self.config.llm.enabled = true;

        // 选择服务商
        let provider = self.prompt_choice(
            "选择 LLM 服务商: ",
            &["OpenAI", "Azure OpenAI", "Anthropic (Claude)", "Google Gemini", "自定义 (OpenAI 兼容)"],
        )?;

        self.config.llm.provider = match provider.as_str() {
            "OpenAI" => {
                self.config.llm.base_url = "https://api.openai.com/v1".to_string();
                "openai".to_string()
            }
            "Azure OpenAI" => {
                let endpoint = self.prompt_input("请输入 Azure Endpoint (如: https://your-resource.openai.azure.com): ", String::new())?;
                self.config.llm.base_url = format!("{}/openai/deployments/your-deployment", endpoint.trim_end_matches('/'));
                "azure_openai".to_string()
            }
            "Anthropic (Claude)" => {
                self.config.llm.base_url = "https://api.anthropic.com/v1".to_string();
                "anthropic".to_string()
            }
            "Google Gemini" => {
                self.config.llm.base_url = "https://generativelanguage.googleapis.com/v1beta".to_string();
                "gemini".to_string()
            }
            "自定义 (OpenAI 兼容)" => {
                self.config.llm.base_url = self.prompt_input("请输入 API Base URL (如: https://api.example.com/v1): ", String::new())?;
                "custom".to_string()
            }
            _ => unreachable!(),
        };

        // API Key
        self.config.llm.api_key = self.prompt_input("请输入 API Key: ", String::new())?;

        // 模型选择
        let default_model = match self.config.llm.provider.as_str() {
            "openai" => "gpt-3.5-turbo",
            "azure_openai" => "gpt-35-turbo",
            "anthropic" => "claude-3-sonnet-20240229",
            "gemini" => "gemini-pro",
            _ => "gpt-3.5-turbo",
        };

        self.config.llm.model = self.prompt_input(
            &format!("模型名称 [{}]: ", default_model),
            default_model.to_string(),
        )?;

        // Temperature
        let temp_str = self.prompt_input("Temperature (0.0 - 2.0, 默认 0.7) [0.7]: ", "0.7".to_string())?;
        self.config.llm.temperature = temp_str.parse::<f32>()
            .map_err(|_| anyhow::anyhow!("无效的数值"))?;

        // Max Tokens
        let tokens_str = self.prompt_input("最大 Tokens 数 (默认 2000) [2000]: ", "2000".to_string())?;
        self.config.llm.max_tokens = tokens_str.parse::<usize>()
            .map_err(|_| anyhow::anyhow!("无效的数值"))?;

        println!();
        println!("LLM 配置:");
        println!("  服务商: {}", self.config.llm.provider);
        println!("  API Key: {}***", &self.config.llm.api_key[..self.config.llm.api_key.len().saturating_sub(8)]);
        println!("  Base URL: {}", self.config.llm.base_url);
        println!("  模型: {}", self.config.llm.model);
        println!("  Temperature: {}", self.config.llm.temperature);
        println!("  Max Tokens: {}", self.config.llm.max_tokens);
        println!();

        self.confirm_or_edit_llm()?;

        Ok(())
    }

    /// 确认或编辑 LLM 配置
    fn confirm_or_edit_llm(&mut self) -> anyhow::Result<()> {
        loop {
            let choice = self.prompt_choice("是否正确? ", &["确认", "重新配置", "禁用 LLM"])?;

            match choice.as_str() {
                "重新配置" => {
                    return self.configure_llm();
                }
                "禁用 LLM" => {
                    self.config.llm.enabled = false;
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }
    }

    /// 配置通道
    fn configure_channels(&mut self) -> anyhow::Result<()> {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  📱 通道配置");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        println!("选择要启用的通道:");
        println!("  [默认预装] Telegram, 钉钉");
        println!();

        let channels = vec![
            ("Telegram ⭐", "telegram"),
            ("Slack", "slack"),
            ("Discord", "discord"),
            ("WhatsApp", "whatsapp"),
            ("钉钉 ⭐", "dingtalk"),
            ("飞书", "feishu"),
            ("企业微信", "wework"),
        ];

        for (i, (name, _)) in channels.iter().enumerate() {
            println!("  {}. {}", i + 1, name);
        }

        println!();
        println!("提示: Telegram 和钉钉为默认预装通道，推荐优先配置");
        println!();

        let choice = self.prompt_choice(
            "选择要配置的通道 (输入序号，多个用空格分隔): ",
            &["1", "2", "3", "4", "5", "6", "7", "继续 (跳过通道配置)"],
        )?;

        match choice.as_str() {
            "继续 (跳过通道配置)" => {
                println!("跳过通道配置");
                return Ok(());
            }
            _ => {
                let indices: Vec<usize> = choice
                    .split_whitespace()
                    .map(|s| s.parse::<usize>().unwrap_or(0))
                    .collect();

                for index in indices {
                    if let Some((name, key)) = channels.get(index.wrapping_sub(1)) {
                        self.configure_channel(key, name)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// 配置单个通道
    fn configure_channel(&mut self, channel_key: &str, channel_name: &str) -> anyhow::Result<()> {
        println!();
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  {} 配置", channel_name);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();

        // 询问是否启用
        let enabled = self.prompt_choice(
            &format!("是否启用 {}? ", channel_name),
            &["是", "否"],
        )?;

        let config = if enabled == "是" {
            let bot_token = match channel_key {
                "dingtalk" => self.prompt_input("请输入 App Key: ", String::new())?,
                "feishu" => self.prompt_input("请输入 App ID: ", String::new())?,
                "wework" => self.prompt_input("请输入 Corp ID: ", String::new())?,
                _ => self.prompt_input("请输入 Bot Token: ", String::new())?,
            };

            let mut extra = std::collections::HashMap::new();

            // 根据通道类型询问额外配置
            match channel_key {
                "telegram" => {
                    let webhook_secret = self.prompt_input("请输入 Webhook Secret (可选): ", String::new())?;
                    if !webhook_secret.is_empty() {
                        extra.insert("webhook_secret".to_string(), webhook_secret);
                    }
                }
                "slack" => {
                    let signing_secret = self.prompt_input("请输入 Signing Secret: ", String::new())?;
                    extra.insert("signing_secret".to_string(), signing_secret);
                }
                "discord" => {
                    let application_id = self.prompt_input("请输入 Application ID: ", String::new())?;
                    extra.insert("application_id".to_string(), application_id);
                }
                "whatsapp" => {
                    let phone_number_id = self.prompt_input("请输入 Phone Number ID: ", String::new())?;
                    extra.insert("phone_number_id".to_string(), phone_number_id);

                    let business_account_id = self.prompt_input("请输入 Business Account ID: ", String::new())?;
                    extra.insert("business_account_id".to_string(), business_account_id);

                    let webhook_verify_token = self.prompt_input("请输入 Webhook Verify Token (可选): ", String::new())?;
                    if !webhook_verify_token.is_empty() {
                        extra.insert("webhook_verify_token".to_string(), webhook_verify_token);
                    }
                }
                "dingtalk" => {
                    let app_secret = self.prompt_input("请输入 App Secret: ", String::new())?;
                    extra.insert("app_secret".to_string(), app_secret);

                    let agent_id = self.prompt_input("请输入 Agent ID: ", String::new())?;
                    extra.insert("agent_id".to_string(), agent_id);
                }
                "feishu" => {
                    let app_secret = self.prompt_input("请输入 App Secret: ", String::new())?;
                    extra.insert("app_secret".to_string(), app_secret);

                    let encrypt_key = self.prompt_input("请输入 Encrypt Key (可选): ", String::new())?;
                    if !encrypt_key.is_empty() {
                        extra.insert("encrypt_key".to_string(), encrypt_key);
                    }

                    let verify_token = self.prompt_input("请输入 Verify Token (可选): ", String::new())?;
                    if !verify_token.is_empty() {
                        extra.insert("verify_token".to_string(), verify_token);
                    }
                }
                "wework" => {
                    let secret = self.prompt_input("请输入 Secret: ", String::new())?;
                    extra.insert("secret".to_string(), secret);

                    let agent_id = self.prompt_input("请输入 Agent ID: ", String::new())?;
                    extra.insert("agent_id".to_string(), agent_id);

                    let token = self.prompt_input("请输入 Token (可选): ", String::new())?;
                    if !token.is_empty() {
                        extra.insert("token".to_string(), token);
                    }

                    let encoding_aes_key = self.prompt_input("请输入 Encoding AES Key (可选): ", String::new())?;
                    if !encoding_aes_key.is_empty() {
                        extra.insert("encoding_aes_key".to_string(), encoding_aes_key);
                    }
                }
                _ => {}
            }

            Some(ChannelConfig {
                enabled: true,
                bot_token: Some(bot_token),
                extra,
            })
        } else {
            None
        };

        // 保存配置
        match channel_key {
            "telegram" => self.config.channels.telegram = config,
            "slack" => self.config.channels.slack = config,
            "discord" => self.config.channels.discord = config,
            "whatsapp" => self.config.channels.whatsapp = config,
            "dingtalk" => self.config.channels.dingtalk = config,
            "feishu" => self.config.channels.feishu = config,
            "wework" => self.config.channels.wework = config,
            _ => {}
        }

        println!();
        println!("✅ {} 配置完成", channel_name);

        Ok(())
    }

    /// 配置安全
    fn configure_security(&mut self) -> anyhow::Result<()> {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  🔒 安全配置");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();

        // JWT 密钥
        println!("JWT 密钥用于签名访问令牌。");
        println!("请使用至少 32 个随机字符。");
        println!();

        let generate_secret = self.prompt_choice(
            "是否自动生成安全的 JWT 密钥? ",
            &["自动生成", "手动输入"],
        )?;

        self.config.security.jwt_secret = match generate_secret.as_str() {
            "自动生成" => {
            // 使用 openssl 生成
            use std::process::Command;
            let output = Command::new("openssl")
                .args(["rand", "-hex", "32"])
                .output()
                .map_err(|_| anyhow::anyhow!("生成密钥失败，请确保已安装 openssl"))?;
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
            "手动输入" => {
            self.prompt_input("请输入 JWT 密钥 (至少 32 字符): ", String::new())?
        }
        _ => unreachable!(),
        };

        // Token 过期时间
        let default_expiry = 86400; // 24 小时
        let expiry_str = self.prompt_input(
            &format!("访问令牌过期时间（秒）[{}]: ", default_expiry),
            default_expiry.to_string(),
        )?;
        self.config.security.token_expiry = expiry_str.parse::<u64>()
            .map_err(|_| anyhow::anyhow!("无效的数字"))?;

        println!();
        println!("安全配置:");
        let jwt_display = if self.config.security.jwt_secret.len() > 20 {
            format!("{}...", &self.config.security.jwt_secret[..20])
        } else {
            self.config.security.jwt_secret.clone()
        };
        println!("  JWT 密钥: {}", jwt_display);
        println!("  过期时间: {} 秒 ({} 小时)",
            self.config.security.token_expiry,
            self.config.security.token_expiry / 3600
        );
        println!();

        self.confirm_or_edit_security()?;

        Ok(())
    }

    /// 确认或编辑安全配置
    fn confirm_or_edit_security(&mut self) -> anyhow::Result<()> {
        loop {
            let choice = self.prompt_choice("是否正确? ", &["确认", "重新配置"])?;

            match choice.as_str() {
                "重新配置" => {
                    return self.configure_security();
                }
                _ => return Ok(()),
            }
        }
    }

    /// 验证配置
    fn validate_config(&self) -> anyhow::Result<()> {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  ✓ 配置验证");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();

        // 验证 JWT 密钥长度
        if self.config.security.jwt_secret.len() < 32 {
            println!("⚠️  警告: JWT 密钥长度少于 32 字符");
            let confirm = self.prompt_choice("继续? ", &["是", "否"])?;
            if confirm == "否" {
                return Err(anyhow::anyhow!("配置未验证通过"));
            }
        }

        // 验证端口范围
        if self.config.server.port < 1024 || self.config.server.port > 65535 {
            return Err(anyhow::anyhow!("端口号超出范围 (1024-65535)"));
        }

        // 验证数据库路径
        if self.config.database.db_type == "sqlite" {
            if let Some(ref path) = self.config.database.path {
                let db_dir = Path::new(path).parent().unwrap_or(Path::new("."));
                if !db_dir.exists() {
                    fs::create_dir_all(db_dir)?;
                    println!("✓ 创建数据库目录: {}", db_dir.display());
                }
            }
        }

        println!("✓ 配置验证通过");
        println!();

        Ok(())
    }

    /// 保存配置
    fn save_config(&self) -> anyhow::Result<()> {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  💾 保存配置");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();

        // 生成 config.toml
        let toml_content = self.generate_toml_config()?;

        let config_path = Path::new(&self.project_dir).join("config.toml");
        fs::write(&config_path, toml_content)?;

        println!("✓ 配置已保存到: {}", config_path.display());

        // 生成 .env 文件
        let env_content = self.generate_env_config();
        let env_path = Path::new(&self.project_dir).join(".env");
        fs::write(&env_path, env_content)?;

        println!("✓ 环境变量已保存到: {}", env_path.display());
        println!();

        Ok(())
    }

    /// 生成 TOML 配置
    fn generate_toml_config(&self) -> anyhow::Result<String> {
        let mut config = String::new();

        config.push_str("# uHorse 配置文件\n");
        config.push_str("# 由配置向导生成\n\n");

        // 服务器配置
        config.push_str("[server]\n");
        config.push_str(&format!("host = \"{}\"\n", self.config.server.host));
        config.push_str(&format!("port = {}\n\n", self.config.server.port));

        // 通道配置
        config.push_str("[channels]\n");
        let mut enabled_channels = Vec::new();

        if let Some(telegram) = &self.config.channels.telegram {
            if telegram.enabled {
                enabled_channels.push("telegram".to_string());
            }
        }
        if let Some(dingtalk) = &self.config.channels.dingtalk {
            if dingtalk.enabled {
                enabled_channels.push("dingtalk".to_string());
            }
        }
        if let Some(slack) = &self.config.channels.slack {
            if slack.enabled {
                enabled_channels.push("slack".to_string());
            }
        }
        if let Some(discord) = &self.config.channels.discord {
            if discord.enabled {
                enabled_channels.push("discord".to_string());
            }
        }
        if let Some(whatsapp) = &self.config.channels.whatsapp {
            if whatsapp.enabled {
                enabled_channels.push("whatsapp".to_string());
            }
        }
        if let Some(feishu) = &self.config.channels.feishu {
            if feishu.enabled {
                enabled_channels.push("feishu".to_string());
            }
        }
        if let Some(wework) = &self.config.channels.wework {
            if wework.enabled {
                enabled_channels.push("wework".to_string());
            }
        }

        if !enabled_channels.is_empty() {
            config.push_str("enabled = [");
            for (i, channel) in enabled_channels.iter().enumerate() {
                if i > 0 {
                    config.push_str(", ");
                }
                config.push_str(&format!("\"{}\"", channel));
            }
            config.push_str("]\n");
        } else {
            config.push_str("enabled = []\n");
        }
        config.push_str("\n");

        // Telegram 配置
        if let Some(telegram) = &self.config.channels.telegram {
            if telegram.enabled {
                config.push_str("[channels.telegram]\n");
                if let Some(token) = &telegram.bot_token {
                    config.push_str(&format!("bot_token = \"{}\"\n", token));
                }
                for (key, value) in &telegram.extra {
                    config.push_str(&format!("{} = \"{}\"\n", key, value));
                }
                config.push_str("\n");
            }
        }

        // 钉钉配置
        if let Some(dingtalk) = &self.config.channels.dingtalk {
            if dingtalk.enabled {
                config.push_str("[channels.dingtalk]\n");
                if let Some(token) = &dingtalk.bot_token {
                    config.push_str(&format!("app_key = \"{}\"\n", token));
                }
                for (key, value) in &dingtalk.extra {
                    config.push_str(&format!("{} = \"{}\"\n", key, value));
                }
                config.push_str("\n");
            }
        }

        // Slack 配置
        if let Some(slack) = &self.config.channels.slack {
            if slack.enabled {
                config.push_str("[channels.slack]\n");
                if let Some(token) = &slack.bot_token {
                    config.push_str(&format!("bot_token = \"{}\"\n", token));
                }
                for (key, value) in &slack.extra {
                    config.push_str(&format!("{} = \"{}\"\n", key, value));
                }
                config.push_str("\n");
            }
        }

        // Discord 配置
        if let Some(discord) = &self.config.channels.discord {
            if discord.enabled {
                config.push_str("[channels.discord]\n");
                if let Some(token) = &discord.bot_token {
                    config.push_str(&format!("bot_token = \"{}\"\n", token));
                }
                for (key, value) in &discord.extra {
                    config.push_str(&format!("{} = \"{}\"\n", key, value));
                }
                config.push_str("\n");
            }
        }

        // WhatsApp 配置
        if let Some(whatsapp) = &self.config.channels.whatsapp {
            if whatsapp.enabled {
                config.push_str("[channels.whatsapp]\n");
                for (key, value) in &whatsapp.extra {
                    config.push_str(&format!("{} = \"{}\"\n", key, value));
                }
                config.push_str("\n");
            }
        }

        // 飞书配置
        if let Some(feishu) = &self.config.channels.feishu {
            if feishu.enabled {
                config.push_str("[channels.feishu]\n");
                if let Some(token) = &feishu.bot_token {
                    config.push_str(&format!("app_id = \"{}\"\n", token));
                }
                for (key, value) in &feishu.extra {
                    config.push_str(&format!("{} = \"{}\"\n", key, value));
                }
                config.push_str("\n");
            }
        }

        // 企业微信配置
        if let Some(wework) = &self.config.channels.wework {
            if wework.enabled {
                config.push_str("[channels.wework]\n");
                if let Some(token) = &wework.bot_token {
                    config.push_str(&format!("corp_id = \"{}\"\n", token));
                }
                for (key, value) in &wework.extra {
                    config.push_str(&format!("{} = \"{}\"\n", key, value));
                }
                config.push_str("\n");
            }
        }

        // 数据库配置
        config.push_str("[database]\n");
        if self.config.database.db_type == "sqlite" {
            config.push_str("path = \"");
            if let Some(ref path) = self.config.database.path {
                config.push_str(path);
            }
            config.push_str("\"\n\n");
        } else {
            config.push_str("[database.postgres]\n");
            if let Some(ref url) = self.config.database.url {
                config.push_str(&format!("url = \"{}\"\n", url));
            }
            config.push_str("\n");
        }

        // 安全配置
        config.push_str("[security]\n");
        config.push_str(&format!("jwt_secret = \"{}\"\n", self.config.security.jwt_secret));
        config.push_str(&format!("token_expiry = {}\n\n", self.config.security.token_expiry));

        // LLM 配置
        if self.config.llm.enabled {
            config.push_str("[llm]\n");
            config.push_str(&format!("enabled = true\n"));
            config.push_str(&format!("provider = \"{}\"\n", self.config.llm.provider));
            config.push_str(&format!("api_key = \"{}\"\n", self.config.llm.api_key));
            config.push_str(&format!("base_url = \"{}\"\n", self.config.llm.base_url));
            config.push_str(&format!("model = \"{}\"\n", self.config.llm.model));
            config.push_str(&format!("temperature = {}\n", self.config.llm.temperature));
            config.push_str(&format!("max_tokens = {}\n\n", self.config.llm.max_tokens));
        }

        Ok(config)
    }

    /// 生成环境变量配置
    fn generate_env_config(&self) -> String {
        let mut env = String::new();

        env.push_str("# uHorse 环境变量\n");
        env.push_str("# 由配置向导生成\n\n");

        env.push_str(&format!("UHORSE_SERVER_HOST={}\n", self.config.server.host));
        env.push_str(&format!("UHORSE_SERVER_PORT={}\n", self.config.server.port));

        if let Some(telegram) = &self.config.channels.telegram {
            if let Some(token) = &telegram.bot_token {
                env.push_str(&format!("UHORSE_TELEGRAM_BOT_TOKEN={}\n", token));
            }
        }

        if let Some(slack) = &self.config.channels.slack {
            if let Some(token) = &slack.bot_token {
                env.push_str(&format!("UHORSE_SLACK_BOT_TOKEN={}\n", token));
            }
            if let Some(secret) = slack.extra.get("signing_secret") {
                env.push_str(&format!("UHORSE_SLACK_SIGNING_SECRET={}\n", secret));
            }
        }

        if let Some(discord) = &self.config.channels.discord {
            if let Some(token) = &discord.bot_token {
                env.push_str(&format!("UHORSE_DISCORD_BOT_TOKEN={}\n", token));
            }
        }

        // LLM 配置
        if self.config.llm.enabled {
            env.push_str(&format!("UHORSE_LLM_ENABLED=true\n"));
            env.push_str(&format!("UHORSE_LLM_PROVIDER={}\n", self.config.llm.provider));
            env.push_str(&format!("UHORSE_LLM_API_KEY={}\n", self.config.llm.api_key));
            env.push_str(&format!("UHORSE_LLM_BASE_URL={}\n", self.config.llm.base_url));
            env.push_str(&format!("UHORSE_LLM_MODEL={}\n", self.config.llm.model));
        }

        env.push_str(&format!("RUST_LOG=info\n"));

        env
    }

    /// 打印后续步骤
    fn print_next_steps(&self) {
        println!("╔════════════════════════════════════════════════╗");
        println!("║                                                ║");
        println!("║     🎉 配置完成！                             ║");
        println!("║                                                ║");
        println!("╚════════════════════════════════════════════════╝");
        println!();
        println!("下一步操作:");
        println!();
        println!("  1️⃣  启动 uHorse:");
        println!("     ./start.sh");
        println!();
        println!("  2️⃣  查看服务状态:");
        println!("     curl http://{}:{}/health/live",
            self.config.server.host, self.config.server.port
        );
        println!();

        if self.config.channels.telegram.is_some()
            || self.config.channels.slack.is_some()
            || self.config.channels.discord.is_some()
            || self.config.channels.whatsapp.is_some()
        {
            println!("  3️⃣  配置通道 Webhook:");
            println!("     请参考 CHANNELS.md 配置各通道的 Webhook URL");
            println!();
        }

        println!("  4️⃣  查看配置:");
        println!("     cat config.toml");
        println!();

        println!("📚 文档:");
        println!("  - 配置指南: CONFIG.md");
        println!("  - API 使用: API.md");
        println!("  - 通道集成: CHANNELS.md");
        println!();

        println!("💡 提示:");
        println!("  - 配置文件已保存到项目根目录");
        println!("  - 可随时编辑 config.toml 或 .env 修改配置");
        println!("  - 重新运行向导: ./target/release/uhorse wizard");
        println!();
    }

    /// 辅助方法：提示输入
    fn prompt_input(&self, prompt: &str, default: String) -> anyhow::Result<String> {
        print!("{}", prompt);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            Ok(default)
        } else {
            Ok(input.to_string())
        }
    }

    /// 辅助方法：提示选择
    fn prompt_choice(&self, prompt: &str, options: &[&str]) -> anyhow::Result<String> {
        loop {
            println!("{}", prompt);
            for (i, option) in options.iter().enumerate() {
                println!("  {}. {}", i + 1, option);
            }
            print!("请选择 [1-{}]: ", options.len());
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if let Ok(index) = input.parse::<usize>() {
                if index >= 1 && index <= options.len() {
                    return Ok(options[index - 1].to_string());
                }
            }

            println!("❌ 无效选择，请重试");
        }
    }

    /// 辅助方法：按 Enter 继续
    fn press_enter_to_continue(&self) {
        println!("按 Enter 继续...");
        let mut input = String::new();
        io::stdin().read_line(&mut input).ok();
    }
}

/// CLI 命令行界面
pub struct CliWizard {
    wizard: ConfigWizard,
}

impl CliWizard {
    pub fn new(project_dir: String) -> Self {
        Self {
            wizard: ConfigWizard::new(project_dir),
        }
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        self.wizard.start()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_input() {
        // 测试需要交互，跳过
    }
}
