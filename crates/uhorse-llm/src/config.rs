//! # LLM 配置

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// LLM 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    /// LLM 服务商
    pub provider: LLMProvider,

    /// API 密钥
    pub api_key: String,

    /// API 基础 URL（用于自定义端点）
    #[serde(default = "default_base_url")]
    pub base_url: String,

    /// 使用的模型
    pub model: String,

    /// 温度 (0.0 - 2.0)
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// 最大 tokens数
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Top-P 采样
    #[serde(default = "default_top_p")]
    pub top_p: f32,

    /// 额外参数
    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            provider: LLMProvider::OpenAI,
            api_key: String::new(),
            base_url: default_base_url(),
            model: "gpt-3.5-turbo".to_string(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            top_p: default_top_p(),
            extra: HashMap::new(),
        }
    }
}

fn default_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_temperature() -> f32 {
    0.7
}

fn default_max_tokens() -> usize {
    2000
}

fn default_top_p() -> f32 {
    1.0
}

/// LLM 服务商
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LLMProvider {
    /// OpenAI
    OpenAI,
    /// Azure OpenAI
    AzureOpenAI,
    /// Anthropic Claude
    Anthropic,
    /// Google Gemini
    Gemini,
    /// OpenAI 兼容的自定义端点
    Custom(String),
}

impl LLMProvider {
    pub fn as_str(&self) -> &str {
        match self {
            LLMProvider::OpenAI => "openai",
            LLMProvider::AzureOpenAI => "azure_openai",
            LLMProvider::Anthropic => "anthropic",
            LLMProvider::Gemini => "gemini",
            LLMProvider::Custom(s) => s,
        }
    }
}

impl std::fmt::Display for LLMProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 预定义的模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMModel {
    /// 模型 ID
    pub id: String,

    /// 模型名称
    pub name: String,

    /// 服务商
    pub provider: LLMProvider,

    /// 上下文长度
    pub context_length: usize,

    /// 是否支持流式输出
    pub supports_streaming: bool,
}

impl LLMModel {
    /// 创建新模型
    pub fn new(
        id: String,
        name: String,
        provider: LLMProvider,
        context_length: usize,
        supports_streaming: bool,
    ) -> Self {
        Self {
            id,
            name,
            provider,
            context_length,
            supports_streaming,
        }
    }

    /// OpenAI GPT-4 Turbo
    pub fn gpt4_turbo() -> Self {
        Self::new(
            "gpt-4-turbo-preview".to_string(),
            "GPT-4 Turbo".to_string(),
            LLMProvider::OpenAI,
            128000,
            true,
        )
    }

    /// OpenAI GPT-3.5 Turbo
    pub fn gpt35_turbo() -> Self {
        Self::new(
            "gpt-3.5-turbo".to_string(),
            "GPT-3.5 Turbo".to_string(),
            LLMProvider::OpenAI,
            16384,
            true,
        )
    }

    /// OpenAI GPT-4
    pub fn gpt4() -> Self {
        Self::new(
            "gpt-4".to_string(),
            "GPT-4".to_string(),
            LLMProvider::OpenAI,
            8192,
            false,
        )
    }
}

/// 常用模型列表
pub fn popular_models() -> Vec<LLMModel> {
    vec![
        LLMModel::gpt35_turbo(),
        LLMModel::gpt4_turbo(),
        LLMModel::gpt4(),
    ]
}
