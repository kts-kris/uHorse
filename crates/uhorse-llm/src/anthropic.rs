//! # Anthropic Claude 客户端
//!
//! 实现 Anthropic API 客户端。

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tracing::{debug, instrument};

use crate::client::{ChatMessage, LLMClient};
use crate::config::LLMConfig;

/// Anthropic API 响应
#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

/// Anthropic Claude 客户端
pub struct AnthropicClient {
    config: LLMConfig,
    client: Client,
}

impl AnthropicClient {
    /// 创建新客户端
    pub fn new(config: LLMConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        Ok(Self { config, client })
    }

    /// 从 API Key 和模型创建客户端
    pub fn from_env(api_key: &str, model: &str) -> Result<Self> {
        let config = LLMConfig {
            provider: crate::config::LLMProvider::Anthropic,
            api_key: api_key.to_string(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            model: model.to_string(),
            ..Default::default()
        };
        Self::new(config)
    }

    /// 构建 API 端点 URL
    fn build_url(&self) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        format!("{}/messages", base)
    }

    /// 将 ChatMessage 转换为 Anthropic 格式
    fn convert_messages(
        &self,
        messages: Vec<ChatMessage>,
    ) -> (Option<String>, Vec<serde_json::Value>) {
        let mut system_prompt = None;
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    system_prompt = Some(msg.content);
                }
                "user" => {
                    anthropic_messages.push(json!({
                        "role": "user",
                        "content": msg.content
                    }));
                }
                "assistant" => {
                    anthropic_messages.push(json!({
                        "role": "assistant",
                        "content": msg.content
                    }));
                }
                _ => {}
            }
        }

        (system_prompt, anthropic_messages)
    }
}

#[async_trait]
impl LLMClient for AnthropicClient {
    #[instrument(skip(self, messages))]
    async fn chat_completion(&self, messages: Vec<ChatMessage>) -> Result<String> {
        debug!("Sending chat completion request to Anthropic");

        let url = self.build_url();
        let (system_prompt, anthropic_messages) = self.convert_messages(messages);

        let mut body = json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "messages": anthropic_messages,
        });

        if let Some(system) = system_prompt {
            body["system"] = json!(system);
        }

        debug!("Request body: {}", serde_json::to_string_pretty(&body)?);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Anthropic API error: {}", error_text));
        }

        let completion: AnthropicResponse = response.json().await?;

        debug!(
            "Received response from Anthropic: {} content blocks",
            completion.content.len()
        );

        // 提取文本内容
        let text = completion
            .content
            .iter()
            .filter_map(|block| {
                if block.block_type == "text" {
                    block.text.clone()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        if text.is_empty() {
            Err(anyhow::anyhow!("No response from Anthropic"))
        } else {
            Ok(text)
        }
    }
}
