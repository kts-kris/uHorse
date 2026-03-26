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

pub mod agent;
pub mod agent_scope;
pub mod bindings;
pub mod error;
pub mod gateway;
pub mod mcp;
pub mod memory;
pub mod router;
pub mod session_key;
pub mod skill;
pub mod skills;
pub mod tools;

// 重新导出核心类型
pub use agent::{Agent, AgentBuilder, AgentConfig, AgentResponse};
pub use agent_scope::{AgentManager, AgentScope, AgentScopeConfig, SessionState};
pub use bindings::{Binding, BindingBuilder, BindingsConfig, BindingsRouter};
pub use error::{AgentError, AgentResult};
pub use gateway::{Gateway, GatewayConfig, GatewayEvent};
pub use memory::{FileMemory, LayeredMemoryStore, MemoryStore};
pub use router::{Route, RouteTarget, Router};
pub use session_key::{ChannelType, SessionKey, SessionNamespace};
pub use skill::{
    LayeredSkillEntry, LayeredSkillRegistry, Skill, SkillExecutor, SkillManifest, SkillRegistry,
};

// MCP 相关
pub use mcp::{
    protocol::McpProtocol,
    types::{McpContent, McpPrompt, McpResource, McpTool, McpToolCall, McpToolResult},
};

// Skills 相关
pub use skills::{
    Skill as McpSkill, SkillConfig, SkillManifestParser, SkillPermission,
    SkillRegistry as McpSkillRegistry,
};

// Tools 相关
pub use tools::{builtin_tools, Tool, ToolBuilder, ToolRegistry};

/// uHorse Agent 框架版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
