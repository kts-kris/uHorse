//! # MCP (Model Context Protocol)
//!
//! OpenClaw 风格的 MCP 协议实现。
//!
//! ## 架构
//!
//! ```text
//! MCP Host ← → MCP Client ← → MCP Server
//!    ↓             ↓            ↓
//!  Agent      Tools/    Resources/
//!           Resources   Prompts
//! ```
//!
//! ## 核心组件
//!
//! - **Tools**: 外部函数调用
//! - **Resources**: 数据源访问
//! - **Prompts**: 提示词模板
//! - **Sessions**: 会话管理

pub mod server;
pub mod client;
pub mod protocol;
pub mod types;

pub use protocol::*;
pub use types::*;
