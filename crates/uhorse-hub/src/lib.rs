//! # uHorse Hub
//!
//! 云端中枢，负责管理节点、调度任务和路由消息。
//!
//! ## 架构位置
//!
//! ```text
//! ┌─────────────────┐                      ┌─────────────────┐
//! │     Hub         │◄──── WebSocket ────►│     Node        │
//! │  (云端中枢)     │                      │   (本地节点)    │
//! └─────────────────┘                      └─────────────────┘
//!        │
//!        ▼
//! ┌─────────────────┐
//! │  消息通道       │
//! │ (钉钉/微信等)   │
//! └─────────────────┘
//! ```
//!
//! ## 模块复用 (3.x → 4.0)
//!
//! | 3.x 模块 | Hub 复用 |
//! |----------|----------|
//! | uhorse-llm | 模型管理与调用 |
//! | uhorse-agent | Agent 编排、技能系统 |
//! | uhorse-channel | 多通道消息接入 |
//! | uhorse-session | 会话管理 |
//! | uhorse-storage | 数据持久化 |
//! | uhorse-security | 安全与认证 |
//! | uhorse-observability | 可观测性 |
//! | uhorse-config | 配置管理 |

#![warn(missing_docs)]
#![warn(clippy::all)]

// Hub 特有模块（4.0 新增）
pub mod error;
pub mod node_manager;
pub mod task_scheduler;
pub mod message_router;
pub mod hub;
pub mod orchestrator;
pub mod security_integration;
pub mod web;

// 重新导出 Hub 特有模块
pub use error::{HubError, HubResult};
pub use node_manager::{NodeManager, NodeInfo, NodeState, NodeManagerStats};
pub use task_scheduler::{TaskScheduler, QueuedTask, ScheduledTask, TaskStatusInfo, SchedulerStats, TaskResult};
pub use message_router::MessageRouter;
pub use hub::{Hub, HubConfig, HubStats};
pub use orchestrator::{Orchestrator, OrchestrationPlan, OrchestrationResult, SubTask, SubTaskResult};
pub use security_integration::{
    NodeAuthenticator, NodeAuthInfo,
    SensitiveOperationApprover,
    HubTlsConfig, HubFieldEncryptor,
    SecurityManager,
};

// Web 管理界面
pub use web::{WebState, WebConfig, ApiResponse, TaskInfo, create_router, start_server};

// 重新导出复用的 3.x 模块（方便下游使用）
pub use uhorse_llm as llm;
pub use uhorse_agent as agent;
pub use uhorse_channel as channel;
pub use uhorse_session as session;
pub use uhorse_storage as storage;
pub use uhorse_security as security;
pub use uhorse_observability as observability;
pub use uhorse_config as config;

/// Hub 版本
pub const HUB_VERSION: &str = env!("CARGO_PKG_VERSION");
