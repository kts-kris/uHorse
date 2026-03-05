//! # STT (语音转文字)
//!
//! 使用 OpenAI Whisper API 进行语音识别。

use async_trait::async_trait;
use reqwest::multipart;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::{MultimodalError, Result};

/// 支持的语言
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// 自动检测
    Auto,
    /// 中文
    #[serde(rename = "zh")]
    Chinese,
    /// 英文
    #[serde(rename = "en")]
    English,
    /// 日文
    #[serde(rename = "ja")]
    Japanese,
    /// 韩文
    #[serde(rename = "ko")]
    Korean,
    /// 法语
    #[serde(rename = "fr")]
    French,
    /// 德语
    #[serde(rename = "de")]
    German,
    /// 西班牙语
    #[serde(rename = "es")]
    Spanish,
}

impl Default for Language {
    fn default() -> Self {
        Self::Auto
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Auto => write!(f, "auto"),
            Language::Chinese => write!(f, "zh"),
            Language::English => write!(f, "en"),
            Language::Japanese => write!(f, "ja"),
            Language::Korean => write!(f, "ko"),
            Language::French => write!(f, "fr"),
            Language::German => write!(f, "de"),
            Language::Spanish => write!(f, "es"),
        }
    }
}

/// Whisper 模型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WhisperModel {
    /// whisper-1 (标准模型)
    #[serde(rename = "whisper-1")]
    Whisper1,
}

impl Default for WhisperModel {
    fn default() -> Self {
        Self::Whisper1
    }
}

impl std::fmt::Display for WhisperModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WhisperModel::Whisper1 => write!(f, "whisper-1"),
        }
    }
}

/// STT 转录结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// 转录文本
    pub text: String,
    /// 检测到的语言
    pub language: Option<String>,
    /// 持续时间（秒）
    pub duration: Option<f32>,
}

/// STT 翻译结果（翻译为英文）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResult {
    /// 翻译文本
    pub text: String,
}

/// STT 配置
#[derive(Debug, Clone)]
pub struct SttConfig {
    /// API Key
    pub api_key: String,
    /// API Base URL
    pub api_base: String,
    /// 模型
    pub model: WhisperModel,
    /// 语言
    pub language: Language,
    /// 超时（秒）
    pub timeout_secs: u64,
}

impl SttConfig {
    /// 创建新配置
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            api_base: "https://api.openai.com/v1".to_string(),
            model: WhisperModel::default(),
            language: Language::default(),
            timeout_secs: 120,
        }
    }

    /// 设置 API Base URL
    pub fn with_api_base(mut self, api_base: String) -> Self {
        self.api_base = api_base;
        self
    }

    /// 设置语言
    pub fn with_language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }
}

/// STT 客户端
#[derive(Debug, Clone)]
pub struct SttClient {
    config: SttConfig,
    http: reqwest::Client,
}

impl SttClient {
    /// 创建新客户端
    pub fn new(config: SttConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, http }
    }

    /// 转录音频
    #[instrument(skip(self, audio_data))]
    pub async fn transcribe(
        &self,
        audio_data: &[u8],
        filename: &str,
    ) -> Result<TranscriptionResult> {
        debug!(
            "Transcribing audio: {} bytes, filename: {}",
            audio_data.len(),
            filename
        );

        let url = format!("{}/audio/transcriptions", self.config.api_base);

        // 构建 multipart 表单
        let file_part = multipart::Part::bytes(audio_data.to_vec())
            .file_name(filename.to_string())
            .mime_str("audio/mpeg")
            .map_err(|e| MultimodalError::ApiError(format!("MIME error: {}", e)))?;

        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.config.model.to_string());

        if self.config.language != Language::Auto {
            form = form.text("language", self.config.language.to_string());
        }

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(MultimodalError::ApiError(format!("API error: {}", error)));
        }

        let result: TranscriptionResult = response.json().await?;
        debug!("Transcription complete: {} chars", result.text.len());

        Ok(result)
    }

    /// 翻译音频（翻译为英文）
    #[instrument(skip(self, audio_data))]
    pub async fn translate(&self, audio_data: &[u8], filename: &str) -> Result<TranslationResult> {
        debug!(
            "Translating audio: {} bytes, filename: {}",
            audio_data.len(),
            filename
        );

        let url = format!("{}/audio/translations", self.config.api_base);

        let file_part = multipart::Part::bytes(audio_data.to_vec())
            .file_name(filename.to_string())
            .mime_str("audio/mpeg")
            .map_err(|e| MultimodalError::ApiError(format!("MIME error: {}", e)))?;

        let form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.config.model.to_string());

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(MultimodalError::ApiError(format!("API error: {}", error)));
        }

        let result: TranslationResult = response.json().await?;
        debug!("Translation complete: {} chars", result.text.len());

        Ok(result)
    }

    /// 转录音频流
    #[instrument(skip(self, audio_stream))]
    pub async fn transcribe_stream(
        &self,
        audio_stream: &[u8],
        filename: &str,
    ) -> Result<TranscriptionResult> {
        // 对于流式音频，先收集完整数据再处理
        self.transcribe(audio_stream, filename).await
    }
}

/// STT 服务 trait
#[async_trait]
pub trait SttService: Send + Sync {
    /// 转录音频
    async fn transcribe(&self, audio_data: &[u8], filename: &str) -> Result<TranscriptionResult>;

    /// 翻译音频
    async fn translate(&self, audio_data: &[u8], filename: &str) -> Result<TranslationResult>;
}

#[async_trait]
impl SttService for SttClient {
    async fn transcribe(&self, audio_data: &[u8], filename: &str) -> Result<TranscriptionResult> {
        self.transcribe(audio_data, filename).await
    }

    async fn translate(&self, audio_data: &[u8], filename: &str) -> Result<TranslationResult> {
        self.translate(audio_data, filename).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_display() {
        assert_eq!(Language::Auto.to_string(), "auto");
        assert_eq!(Language::Chinese.to_string(), "zh");
        assert_eq!(Language::English.to_string(), "en");
    }

    #[test]
    fn test_model_display() {
        assert_eq!(WhisperModel::Whisper1.to_string(), "whisper-1");
    }

    #[test]
    fn test_config_builder() {
        let config = SttConfig::new("test-key".to_string())
            .with_api_base("https://custom.api.com".to_string())
            .with_language(Language::Chinese);

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.api_base, "https://custom.api.com");
        assert_eq!(config.language, Language::Chinese);
    }
}
