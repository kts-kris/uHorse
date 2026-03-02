//! # OpenClaw Channel
//!
//! 通道适配器层，支持 Telegram、Slack、Discord、WhatsApp。

pub mod telegram;
pub mod slack;
pub mod discord;
pub mod whatsapp;

pub use telegram::TelegramChannel;
pub use slack::SlackChannel;
pub use discord::DiscordChannel;
pub use whatsapp::WhatsAppChannel;
