//! # uHorse LLM 适配器
//!
//! 提供 OpenAI 兼容的大语言模型集成。

pub mod client;
pub mod config;

pub use client::{LLMClient, OpenAIClient, ChatMessage};
pub use config::{LLMConfig, LLMProvider, LLMModel};
