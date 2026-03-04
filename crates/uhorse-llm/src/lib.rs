//! # uHorse LLM 适配器
//!
//! 提供多 LLM 提供商集成（OpenAI、Anthropic、Gemini）。

pub mod anthropic;
pub mod client;
pub mod config;
pub mod gemini;

pub use anthropic::AnthropicClient;
pub use client::{ChatCompletion, ChatMessage, Choice, LLMClient, OpenAIClient, Usage};
pub use config::{LLMConfig, LLMModel, LLMProvider};
pub use gemini::GeminiClient;
