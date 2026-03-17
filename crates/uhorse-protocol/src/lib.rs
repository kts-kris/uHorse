//! # uHorse Protocol
//!
//! Hub-Node 通信协议，定义云端中枢与本地节点之间的消息格式。
//!
//! ## 架构概览
//!
//! ```text
//! ┌─────────────┐                      ┌─────────────┐
//! │    Hub      │◄──── WebSocket ────►│    Node     │
//! │ (云端中枢)  │                      │ (本地节点)  │
//! └─────────────┘                      └─────────────┘
//!       │                                    │
//!       │ 任务分配/取消                       │
//!       │ 心跳/配置更新                       │
//!       ▼                                    ▼
//!       │                                    │
//!       │ 任务结果/进度                       │
//!       │ 错误报告/审批请求                   │
//!       └────────────────────────────────────┘
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod error;
pub mod types;
pub mod command;
pub mod message;
pub mod result;

pub use error::{ProtocolError, ProtocolResult};
pub use types::*;
pub use command::*;
pub use message::*;
pub use result::*;

/// 协议版本
pub const PROTOCOL_VERSION: &str = env!("CARGO_PKG_VERSION");
