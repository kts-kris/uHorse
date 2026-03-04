//! # Google Gemini 客户端
//!
//! 实现 Google Gemini API 客户端。

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tracing::{debug, instrument};

use crate::client::{ChatMessage, LLMClient};
use crate::config::LLMConfig;

/// Gemini API 响应
#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    usage_metadata: Option<GeminiUsage>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
    role: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiPart {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiUsage {
    prompt_token_count: u32,
    candidates_token_count: u32,
    total_token_count: u32,
}

/// Google Gemini 客户端
pub struct GeminiClient {
    config: LLMConfig,
    client: Client,
}

impl GeminiClient {
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
            provider: crate::config::LLMProvider::Gemini,
            api_key: api_key.to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            model: model.to_string(),
            ..Default::default()
        };
        Self::new(config)
    }

    /// 构建 API 端点 URL
    fn build_url(&self) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        format!(
            "{}/models/{}:generateContent?key={}",
            base, self.config.model, self.config.api_key
        )
    }

    /// 将 ChatMessage 转换为 Gemini 格式
    fn convert_messages(&self, messages: Vec<ChatMessage>) -> (Option<String>, Vec<serde_json::Value>) {
        let mut system_instruction = None;
        let mut gemini_messages = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    system_instruction = Some(msg.content);
                }
                "user" => {
                    gemini_messages.push(json!({
                        "role": "user",
                        "parts": [{"text": msg.content}]
                    }));
                }
                "assistant" => {
                    gemini_messages.push(json!({
                        "role": "model",
                        "parts": [{"text": msg.content}]
                    }));
                }
                _ => {}
            }
        }

        (system_instruction, gemini_messages)
    }

    /// 构建生成配置
    fn build_generation_config(&self) -> serde_json::Value {
        json!({
            "temperature": self.config.temperature,
            "maxOutputTokens": self.config.max_tokens,
            "topP": self.config.top_p,
        })
    }
}

#[async_trait]
impl LLMClient for GeminiClient {
    #[instrument(skip(self, messages))]
    async fn chat_completion(&self, messages: Vec<ChatMessage>) -> Result<String> {
        debug!("Sending chat completion request to Gemini");

        let url = self.build_url();
        let (system_instruction, gemini_messages) = self.convert_messages(messages);

        let mut body = json!({
            "contents": gemini_messages,
            "generationConfig": self.build_generation_config(),
        });

        if let Some(system) = system_instruction {
            body["system_instruction"] = json!({
                "parts": [{"text": system}]
            });
        }

        debug!("Request body: {}", serde_json::to_string_pretty(&body)?);

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Gemini API error: {}", error_text));
        }

        let completion: GeminiResponse = response.json().await?;

        debug!("Received response from Gemini: {} candidates", completion.candidates.len());

        // 提取文本内容
        if let Some(candidate) = completion.candidates.first() {
            let text = candidate.content
                .parts
                .iter()
                .filter_map(|part| part.text.clone())
                .collect::<Vec<_>>()
                .join("");

            if text.is_empty() {
                Err(anyhow::anyhow!("No response from Gemini"))
            } else {
                Ok(text)
            }
        } else {
            Err(anyhow::anyhow!("No candidates in Gemini response"))
        }
    }
}
