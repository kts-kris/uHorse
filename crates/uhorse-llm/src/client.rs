//! # LLM 客户端
//!
//! 实现 OpenAI 兼容的 API 客户端

use crate::config::{LLMConfig, LLMProvider};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tracing::{debug, info, instrument};

/// 从 uHorse 配置转换
impl TryFrom<uhorse_config::LLMConfig> for LLMConfig {
    type Error = anyhow::Error;

    fn try_from(config: uhorse_config::LLMConfig) -> Result<Self> {
        let provider = match config.provider.to_lowercase().as_str() {
            "openai" => LLMProvider::OpenAI,
            "azure_openai" => LLMProvider::AzureOpenAI,
            "anthropic" => LLMProvider::Anthropic,
            "gemini" => LLMProvider::Gemini,
            custom => LLMProvider::Custom(custom.to_string()),
        };

        Ok(Self {
            provider,
            api_key: config.api_key,
            base_url: config.base_url,
            model: config.model,
            temperature: config.temperature,
            max_tokens: config.max_tokens,
            top_p: 1.0,
            extra: Default::default(),
        })
    }
}

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// 角色：system, user, assistant
    pub role: String,
    /// 消息内容
    pub content: String,
}

impl ChatMessage {
    /// 创建用户消息
    pub fn user(content: String) -> Self {
        Self {
            role: "user".to_string(),
            content,
        }
    }

    /// 创建助手消息
    pub fn assistant(content: String) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
        }
    }

    /// 创建系统消息
    pub fn system(content: String) -> Self {
        Self {
            role: "system".to_string(),
            content,
        }
    }
}

/// 聊天完成响应
#[derive(Debug, Deserialize)]
pub struct ChatCompletion {
    /// 模型生成的消息
    pub choices: Vec<Choice>,
    /// 使用的 token 数量
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    /// 消息内容
    pub message: ChatMessage,
    /// 完成原因
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    /// 提示 token 数
    pub prompt_tokens: u32,
    /// 完成 token 数
    pub completion_tokens: u32,
    /// 总 token 数
    pub total_tokens: u32,
}

/// LLM 客户端 Trait
#[async_trait]
pub trait LLMClient: Send + Sync {
    /// 发送聊天请求
    async fn chat_completion(&self, messages: Vec<ChatMessage>) -> Result<String>;
}

/// OpenAI 兼容客户端
pub struct OpenAIClient {
    config: LLMConfig,
    client: Client,
}

impl OpenAIClient {
    /// 创建新客户端
    pub fn new(config: LLMConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        Ok(Self { config, client })
    }

    /// 从环境变量创建客户端
    pub fn from_env(api_key: &str, model: &str) -> Result<Self> {
        let config = LLMConfig {
            provider: LLMProvider::OpenAI,
            api_key: api_key.to_string(),
            model: model.to_string(),
            ..Default::default()
        };
        Self::new(config)
    }

    /// 从 uHorse 配置创建客户端
    pub fn from_uhorse_config(config: uhorse_config::LLMConfig) -> Result<Self> {
        let llm_config = LLMConfig::try_from(config)?;
        Self::new(llm_config)
    }

    /// 构建 API 端点 URL
    fn build_url(&self) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        format!("{}/chat/completions", base)
    }

    /// 获取请求头
    fn get_headers(&self) -> Vec<(&'static str, String)> {
        match self.config.provider {
            LLMProvider::OpenAI | LLMProvider::AzureOpenAI => vec![
                ("Authorization", format!("Bearer {}", self.config.api_key)),
                ("Content-Type", "application/json".to_string()),
            ],
            LLMProvider::Anthropic => vec![
                ("x-api-key", self.config.api_key.clone()),
                ("Content-Type", "application/json".to_string()),
                ("anthropic-version", "2023-06-01".to_string()),
            ],
            LLMProvider::Gemini => vec![
                ("Authorization", format!("Bearer {}", self.config.api_key)),
                ("Content-Type", "application/json".to_string()),
            ],
            LLMProvider::Custom(_) => vec![
                ("Authorization", format!("Bearer {}", self.config.api_key)),
                ("Content-Type", "application/json".to_string()),
            ],
        }
    }

    /// 构建请求体
    fn build_request_body(&self, messages: Vec<ChatMessage>) -> Value {
        let mut body = json!({
            "model": self.config.model,
            "messages": messages,
            "temperature": self.config.temperature,
            "max_tokens": self.config.max_tokens,
        });

        // 添加额外参数
        for (key, value) in &self.config.extra {
            body[key] = value.clone();
        }

        body
    }
}

#[async_trait]
impl LLMClient for OpenAIClient {
    #[instrument(skip(self, messages))]
    async fn chat_completion(&self, messages: Vec<ChatMessage>) -> Result<String> {
        debug!("Sending chat completion request to {}", self.config.provider);

        let url = self.build_url();
        let body = self.build_request_body(messages);

        debug!("Request body: {}", serde_json::to_string_pretty(&body)?);

        let mut request = self.client.post(&url);
        for (key, value) in self.get_headers() {
            request = request.header(key, value);
        }

        let response = request.json(&body).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("API error: {}", error_text));
        }

        let completion: ChatCompletion = response.json().await?;

        debug!("Received response: {} choices", completion.choices.len());

        if let Some(choice) = completion.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(anyhow::anyhow!("No response from LLM"))
        }
    }
}
