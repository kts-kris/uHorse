//! # uHorse Channel
//!
//! 通道适配器层，支持 Telegram、Slack、Discord、WhatsApp、钉钉、飞书、企业微信。

pub mod dingtalk;
pub mod discord;
pub mod feishu;
pub mod slack;
pub mod telegram;
pub mod wework;
pub mod whatsapp;

pub use dingtalk::DingTalkChannel;
pub use discord::DiscordChannel;
pub use feishu::FeishuChannel;
pub use slack::SlackChannel;
pub use telegram::TelegramChannel;
pub use wework::WeWorkChannel;
pub use whatsapp::WhatsAppChannel;
