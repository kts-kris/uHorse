//! # uHorse Multi-modal Support
//!
//! 多模态支持模块：
//! - STT (语音转文字) - OpenAI Whisper API
//! - TTS (文字转语音) - OpenAI TTS API
//! - Vision (图像理解) - GPT-4V / Claude Vision
//! - Document (文档解析) - PDF/Word/Excel/Markdown

pub mod stt;
pub mod tts;
pub mod vision;
pub mod document;
mod error;

pub use error::{MultimodalError, Result};
