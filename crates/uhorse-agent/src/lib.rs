//! # uHorse Agent Framework
//!
//! 基于 OpenClaw 四层架构（Gateway-Agent-Skills-Memory）的多智能体系统。
//!
//! ## 架构概览
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        Gateway (控制平面)                        │
//! │  - 会话管理  - 消息路由  - 多通道统一接口  - 事件驱动           │
//! └─────────────────────────────────────────────────────────────────┘
//!                               ↓
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                         Agent (智能体)                          │
//! │  - LLM 调用  - 工具使用  - 意图识别  - 多 Agent 协作            │
//! └─────────────────────────────────────────────────────────────────┘
//!                               ↓
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        Skills (技能系统)                        │
//! │  - SKILL.md 描述  - Rust/WASM 执行  - 参数验证  - 权限控制     │
//! └─────────────────────────────────────────────────────────────────┘
//!                               ↓
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        Memory (记忆系统)                        │
//! │  - MEMORY.md  - SOUL.md  - USER.md  - 文件系统 + SQLite       │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod gateway;
pub mod agent;
pub mod skill;
pub mod memory;
pub mod router;
pub mod error;
pub mod agent_scope;
pub mod session_key;
pub mod bindings;
pub mod mcp;
pub mod skills;
pub mod tools;

// 重新导出核心类型
pub use gateway::{Gateway, GatewayConfig, GatewayEvent};
pub use agent::{Agent, AgentBuilder, AgentConfig, AgentResponse};
pub use skill::{Skill, SkillRegistry, SkillManifest, SkillExecutor};
pub use memory::{FileMemory, MemoryStore};
pub use router::{Router, Route, RouteTarget};
pub use error::{AgentError, AgentResult};
pub use agent_scope::{AgentScope, AgentScopeConfig, AgentManager, SessionState};
pub use session_key::{SessionKey, ChannelType};
pub use bindings::{Binding, BindingsConfig, BindingsRouter, BindingBuilder};

// MCP 相关
pub use mcp::{
    types::{McpTool, McpToolCall, McpToolResult, McpContent, McpResource, McpPrompt},
    protocol::McpProtocol,
};

// Skills 相关
pub use skills::{
    SkillManifestParser, SkillConfig, SkillPermission,
    Skill as McpSkill, SkillRegistry as McpSkillRegistry,
};

// Tools 相关
pub use tools::{Tool, ToolRegistry, ToolBuilder, builtin_tools};

/// uHorse Agent 框架版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
