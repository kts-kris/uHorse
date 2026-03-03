//! # uHorse Channel
//!
//! 通道适配器层，支持 Telegram、Slack、Discord、WhatsApp、钉钉、飞书、企业微信。

pub mod telegram;
pub mod slack;
pub mod discord;
pub mod whatsapp;
pub mod dingtalk;
pub mod feishu;
pub mod wework;

pub use telegram::TelegramChannel;
pub use slack::SlackChannel;
pub use discord::DiscordChannel;
pub use whatsapp::WhatsAppChannel;
pub use dingtalk::DingTalkChannel;
pub use feishu::FeishuChannel;
pub use wework::WeWorkChannel;
