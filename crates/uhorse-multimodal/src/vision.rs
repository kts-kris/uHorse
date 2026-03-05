//! # Vision (图像理解)
//!
//! 使用 GPT-4V / Claude Vision 进行图像理解。

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::{MultimodalError, Result};

/// Vision 模型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VisionModel {
    /// GPT-4 Vision
    #[serde(rename = "gpt-4-vision-preview")]
    Gpt4Vision,
    /// GPT-4 Turbo Vision
    #[serde(rename = "gpt-4-turbo")]
    Gpt4Turbo,
    /// GPT-4o
    #[serde(rename = "gpt-4o")]
    Gpt4o,
    /// Claude 3 Opus Vision
    #[serde(rename = "claude-3-opus")]
    Claude3Opus,
    /// Claude 3.5 Sonnet Vision
    #[serde(rename = "claude-3-5-sonnet")]
    Claude35Sonnet,
}

impl Default for VisionModel {
    fn default() -> Self {
        Self::Gpt4o
    }
}

impl std::fmt::Display for VisionModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VisionModel::Gpt4Vision => write!(f, "gpt-4-vision-preview"),
            VisionModel::Gpt4Turbo => write!(f, "gpt-4-turbo"),
            VisionModel::Gpt4o => write!(f, "gpt-4o"),
            VisionModel::Claude3Opus => write!(f, "claude-3-opus"),
            VisionModel::Claude35Sonnet => write!(f, "claude-3-5-sonnet"),
        }
    }
}

/// 图像内容类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageContent {
    /// 图像 URL 或 base64 数据
    pub url: Option<String>,
    /// Base64 编码的图像数据
    pub base64: Option<String>,
    /// MIME 类型
    pub mime_type: String,
}

impl ImageContent {
    /// 从 URL 创建
    pub fn from_url(url: String, mime_type: String) -> Self {
        Self {
            url: Some(url),
            base64: None,
            mime_type,
        }
    }

    /// 从 Base64 创建
    pub fn from_base64(data: String, mime_type: String) -> Self {
        Self {
            url: None,
            base64: Some(data),
            mime_type,
        }
    }

    /// 从字节创建
    pub fn from_bytes(data: &[u8], mime_type: String) -> Self {
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        Self::from_base64(encoded, mime_type)
    }
}

/// Vision 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionRequest {
    /// 模型
    pub model: String,
    /// 消息
    pub messages: Vec<VisionMessage>,
    /// 最大 Token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

/// Vision 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionMessage {
    /// 角色
    pub role: String,
    /// 内容
    pub content: VisionMessageContent,
}

/// Vision 消息内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VisionMessageContent {
    /// 文本内容
    Text(String),
    /// 多部分内容
    Parts(Vec<ContentPart>),
}

/// 内容部分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPart {
    /// 类型
    #[serde(rename = "type")]
    pub part_type: String,
    /// 文本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 图像 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<ImageUrl>,
}

impl ContentPart {
    /// 创建文本部分
    pub fn text(text: String) -> Self {
        Self {
            part_type: "text".to_string(),
            text: Some(text),
            image_url: None,
        }
    }

    /// 创建图像部分
    pub fn image(url: String) -> Self {
        Self {
            part_type: "image_url".to_string(),
            text: None,
            image_url: Some(ImageUrl { url }),
        }
    }

    /// 创建 Base64 图像部分
    pub fn image_base64(data: String, mime_type: &str) -> Self {
        Self {
            part_type: "image_url".to_string(),
            text: None,
            image_url: Some(ImageUrl {
                url: format!("data:{};base64,{}", mime_type, data),
            }),
        }
    }
}

/// 图像 URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    /// URL 或 data URI
    pub url: String,
}

/// Vision 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionResponse {
    /// ID
    pub id: String,
    /// 对象类型
    pub object: String,
    /// 创建时间
    pub created: i64,
    /// 模型
    pub model: String,
    /// 选择
    pub choices: Vec<VisionChoice>,
    /// 使用量
    pub usage: Option<VisionUsage>,
}

/// Vision 选择
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionChoice {
    /// 索引
    pub index: u32,
    /// 消息
    pub message: VisionResponseMessage,
    /// 完成原因
    pub finish_reason: Option<String>,
}

/// Vision 响应消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionResponseMessage {
    /// 角色
    pub role: String,
    /// 内容
    pub content: Option<String>,
}

/// Vision 使用量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionUsage {
    /// 提示 Token
    pub prompt_tokens: u32,
    /// 完成 Token
    pub completion_tokens: u32,
    /// 总 Token
    pub total_tokens: u32,
}

/// Vision 分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// 描述
    pub description: String,
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 置信度 (0.0 - 1.0)
    #[serde(default)]
    pub confidence: f32,
}

/// Vision 配置
#[derive(Debug, Clone)]
pub struct VisionConfig {
    /// API Key
    pub api_key: String,
    /// API Base URL
    pub api_base: String,
    /// 模型
    pub model: VisionModel,
    /// 最大 Token
    pub max_tokens: u32,
    /// 超时（秒）
    pub timeout_secs: u64,
}

impl VisionConfig {
    /// 创建新配置
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            api_base: "https://api.openai.com/v1".to_string(),
            model: VisionModel::default(),
            max_tokens: 4096,
            timeout_secs: 60,
        }
    }

    /// 设置模型
    pub fn with_model(mut self, model: VisionModel) -> Self {
        self.model = model;
        self
    }

    /// 设置 API Base URL
    pub fn with_api_base(mut self, api_base: String) -> Self {
        self.api_base = api_base;
        self
    }
}

/// Vision 客户端
#[derive(Debug, Clone)]
pub struct VisionClient {
    config: VisionConfig,
    http: reqwest::Client,
}

impl VisionClient {
    /// 创建新客户端
    pub fn new(config: VisionConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, http }
    }

    /// 分析图像
    #[instrument(skip(self, image_data))]
    pub async fn analyze(&self, image_data: &[u8], prompt: &str, mime_type: &str) -> Result<VisionResponse> {
        debug!("Analyzing image: {} bytes, mime: {}", image_data.len(), mime_type);

        let url = format!("{}/chat/completions", self.config.api_base);

        // 编码图像
        let encoded = base64::engine::general_purpose::STANDARD.encode(image_data);

        let request = VisionRequest {
            model: self.config.model.to_string(),
            messages: vec![VisionMessage {
                role: "user".to_string(),
                content: VisionMessageContent::Parts(vec![
                    ContentPart::text(prompt.to_string()),
                    ContentPart::image_base64(encoded, mime_type),
                ]),
            }],
            max_tokens: Some(self.config.max_tokens),
        };

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(MultimodalError::ApiError(format!("API error: {}", error)));
        }

        let result: VisionResponse = response.json().await?;
        debug!("Vision analysis complete");

        Ok(result)
    }

    /// 从 URL 分析图像
    #[instrument(skip(self))]
    pub async fn analyze_url(&self, image_url: &str, prompt: &str) -> Result<VisionResponse> {
        debug!("Analyzing image from URL: {}", image_url);

        let url = format!("{}/chat/completions", self.config.api_base);

        let request = VisionRequest {
            model: self.config.model.to_string(),
            messages: vec![VisionMessage {
                role: "user".to_string(),
                content: VisionMessageContent::Parts(vec![
                    ContentPart::text(prompt.to_string()),
                    ContentPart::image(image_url.to_string()),
                ]),
            }],
            max_tokens: Some(self.config.max_tokens),
        };

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(MultimodalError::ApiError(format!("API error: {}", error)));
        }

        let result: VisionResponse = response.json().await?;
        debug!("Vision analysis complete");

        Ok(result)
    }
}

/// Vision 服务 trait
#[async_trait]
pub trait VisionService: Send + Sync {
    /// 分析图像
    async fn analyze(&self, image_data: &[u8], prompt: &str, mime_type: &str) -> Result<VisionResponse>;

    /// 从 URL 分析图像
    async fn analyze_url(&self, image_url: &str, prompt: &str) -> Result<VisionResponse>;
}

#[async_trait]
impl VisionService for VisionClient {
    async fn analyze(&self, image_data: &[u8], prompt: &str, mime_type: &str) -> Result<VisionResponse> {
        self.analyze(image_data, prompt, mime_type).await
    }

    async fn analyze_url(&self, image_url: &str, prompt: &str) -> Result<VisionResponse> {
        self.analyze_url(image_url, prompt).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_display() {
        assert_eq!(VisionModel::Gpt4o.to_string(), "gpt-4o");
        assert_eq!(VisionModel::Gpt4Vision.to_string(), "gpt-4-vision-preview");
    }

    #[test]
    fn test_content_part_text() {
        let part = ContentPart::text("Hello".to_string());
        assert_eq!(part.part_type, "text");
        assert_eq!(part.text, Some("Hello".to_string()));
        assert!(part.image_url.is_none());
    }

    #[test]
    fn test_content_part_image() {
        let part = ContentPart::image("https://example.com/image.png".to_string());
        assert_eq!(part.part_type, "image_url");
        assert!(part.text.is_none());
        assert!(part.image_url.is_some());
    }

    #[test]
    fn test_image_content_from_bytes() {
        let data = b"fake image data";
        let content = ImageContent::from_bytes(data, "image/png".to_string());
        assert!(content.url.is_none());
        assert!(content.base64.is_some());
        assert_eq!(content.mime_type, "image/png");
    }
}
