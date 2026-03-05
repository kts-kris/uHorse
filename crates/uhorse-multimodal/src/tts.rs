//! # TTS (文字转语音)
//!
//! 使用 OpenAI TTS API 进行语音合成。

use async_trait::async_trait;
use reqwest::multipart;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::{MultimodalError, Result};

/// TTS 模型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TtsModel {
    /// tts-1 (标准质量)
    #[serde(rename = "tts-1")]
    Tts1,
    /// tts-1-hd (高清质量)
    #[serde(rename = "tts-1-hd")]
    Tts1Hd,
}

impl Default for TtsModel {
    fn default() -> Self {
        Self::Tts1
    }
}

impl std::fmt::Display for TtsModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TtsModel::Tts1 => write!(f, "tts-1"),
            TtsModel::Tts1Hd => write!(f, "tts-1-hd"),
        }
    }
}

/// TTS 音色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Voice {
    /// Alloy (中性)
    Alloy,
    /// Echo (男性)
    Echo,
    /// Fable (英式男性)
    Fable,
    /// Onyx (深沉男性)
    Onyx,
    /// Nova (女性)
    Nova,
    /// Shimmer (柔和女性)
    Shimmer,
}

impl Default for Voice {
    fn default() -> Self {
        Self::Alloy
    }
}

impl std::fmt::Display for Voice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Voice::Alloy => write!(f, "alloy"),
            Voice::Echo => write!(f, "echo"),
            Voice::Fable => write!(f, "fable"),
            Voice::Onyx => write!(f, "onyx"),
            Voice::Nova => write!(f, "nova"),
            Voice::Shimmer => write!(f, "shimmer"),
        }
    }
}

/// 音频格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    /// MP3
    Mp3,
    /// Opus
    Opus,
    /// AAC
    Aac,
    /// FLAC
    Flac,
    /// WAV
    Wav,
    /// PCM
    Pcm,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self::Mp3
    }
}

impl std::fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioFormat::Mp3 => write!(f, "mp3"),
            AudioFormat::Opus => write!(f, "opus"),
            AudioFormat::Aac => write!(f, "aac"),
            AudioFormat::Flac => write!(f, "flac"),
            AudioFormat::Wav => write!(f, "wav"),
            AudioFormat::Pcm => write!(f, "pcm"),
        }
    }
}

/// TTS 配置
#[derive(Debug, Clone)]
pub struct TtsConfig {
    /// API Key
    pub api_key: String,
    /// API Base URL
    pub api_base: String,
    /// 模型
    pub model: TtsModel,
    /// 音色
    pub voice: Voice,
    /// 输出格式
    pub format: AudioFormat,
    /// 语速 (0.25 - 4.0)
    pub speed: f32,
    /// 超时（秒）
    pub timeout_secs: u64,
}

impl TtsConfig {
    /// 创建新配置
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            api_base: "https://api.openai.com/v1".to_string(),
            model: TtsModel::default(),
            voice: Voice::default(),
            format: AudioFormat::default(),
            speed: 1.0,
            timeout_secs: 120,
        }
    }

    /// 设置 API Base URL
    pub fn with_api_base(mut self, api_base: String) -> Self {
        self.api_base = api_base;
        self
    }

    /// 设置模型
    pub fn with_model(mut self, model: TtsModel) -> Self {
        self.model = model;
        self
    }

    /// 设置音色
    pub fn with_voice(mut self, voice: Voice) -> Self {
        self.voice = voice;
        self
    }

    /// 设置语速
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed.clamp(0.25, 4.0);
        self
    }

    /// 设置格式
    pub fn with_format(mut self, format: AudioFormat) -> Self {
        self.format = format;
        self
    }
}

/// TTS 客户端
#[derive(Debug, Clone)]
pub struct TtsClient {
    config: TtsConfig,
    http: reqwest::Client,
}

impl TtsClient {
    /// 创建新客户端
    pub fn new(config: TtsConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, http }
    }

    /// 合成语音
    #[instrument(skip(self, text))]
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        debug!("Synthesizing speech: {} chars", text.len());

        // 文本长度限制
        if text.len() > 4096 {
            return Err(MultimodalError::ApiError(
                "Text exceeds maximum length of 4096 characters".to_string(),
            ));
        }

        let url = format!("{}/audio/speech", self.config.api_base);

        #[derive(Serialize)]
        struct SpeechRequest {
            model: String,
            input: String,
            voice: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            response_format: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            speed: Option<f32>,
        }

        let request = SpeechRequest {
            model: self.config.model.to_string(),
            input: text.to_string(),
            voice: self.config.voice.to_string(),
            response_format: Some(self.config.format.to_string()),
            speed: if self.config.speed != 1.0 {
                Some(self.config.speed)
            } else {
                None
            },
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

        let audio_data = response.bytes().await?;
        debug!("Speech synthesis complete: {} bytes", audio_data.len());

        Ok(audio_data.to_vec())
    }

    /// 流式合成语音（返回流式响应）
    #[instrument(skip(self, text))]
    pub async fn synthesize_stream(&self, text: &str) -> Result<Vec<u8>> {
        // OpenAI TTS API 支持流式响应，但这里简化处理
        self.synthesize(text).await
    }
}

/// TTS 服务 trait
#[async_trait]
pub trait TtsService: Send + Sync {
    /// 合成语音
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>>;
}

#[async_trait]
impl TtsService for TtsClient {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        self.synthesize(text).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_display() {
        assert_eq!(TtsModel::Tts1.to_string(), "tts-1");
        assert_eq!(TtsModel::Tts1Hd.to_string(), "tts-1-hd");
    }

    #[test]
    fn test_voice_display() {
        assert_eq!(Voice::Alloy.to_string(), "alloy");
        assert_eq!(Voice::Nova.to_string(), "nova");
        assert_eq!(Voice::Echo.to_string(), "echo");
    }

    #[test]
    fn test_format_display() {
        assert_eq!(AudioFormat::Mp3.to_string(), "mp3");
        assert_eq!(AudioFormat::Wav.to_string(), "wav");
    }

    #[test]
    fn test_config_builder() {
        let config = TtsConfig::new("test-key".to_string())
            .with_model(TtsModel::Tts1Hd)
            .with_voice(Voice::Nova)
            .with_speed(1.5)
            .with_format(AudioFormat::Opus);

        assert_eq!(config.model, TtsModel::Tts1Hd);
        assert_eq!(config.voice, Voice::Nova);
        assert_eq!(config.speed, 1.5);
        assert_eq!(config.format, AudioFormat::Opus);
    }

    #[test]
    fn test_speed_clamp() {
        let config = TtsConfig::new("test-key".to_string()).with_speed(10.0);
        assert_eq!(config.speed, 4.0);

        let config = TtsConfig::new("test-key".to_string()).with_speed(0.1);
        assert_eq!(config.speed, 0.25);
    }
}
