//! # uHorse Node
//!
//! 本地执行节点，负责接收云端中枢下发的命令并在本地执行。
//!
//! ## 架构位置
//!
//! ```text
//! ┌─────────────────┐                      ┌─────────────────┐
//! │     Hub         │◄──── WebSocket ────►│     Node        │
//! │  (云端中枢)     │                      │   (本地节点)    │
//! └─────────────────┘                      └─────────────────┘
//!                                                  │
//!                                                  ▼
//!                                         ┌─────────────────┐
//!                                         │   Workspace     │
//!                                         │   工作目录      │
//!                                         └─────────────────┘
//! ```
//!
//! ## 核心功能
//!
//! - **工作空间管理**: 管理用户授权的工作目录
//! - **权限控制**: 检查并限制操作权限
//! - **命令执行**: 执行 Hub 下发的各类命令
//! - **状态报告**: 定期向 Hub 上报节点状态
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use uhorse_node::{Node, NodeConfig, ConnectionConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 创建节点配置
//!     let config = NodeConfig {
//!         name: "my-node".to_string(),
//!         connection: ConnectionConfig {
//!             hub_url: "wss://hub.uhorse.ai".to_string(),
//!             ..Default::default()
//!         },
//!         workspace_path: "/Users/xxx/projects".to_string(),
//!         ..Default::default()
//!     };
//!
//!     // 启动节点
//!     let mut node = Node::new(config)?;
//!     node.start().await?;
//!
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub use uhorse_node_runtime::connection;
pub use uhorse_node_runtime::error;
pub use uhorse_node_runtime::executor;
pub use uhorse_node_runtime::node;
pub use uhorse_node_runtime::permission;
pub use uhorse_node_runtime::status;
pub mod tools;
pub use uhorse_node_runtime::workspace;

pub use tools::{BrowserExecutor, DatabaseExecutor, SkillExecutor};
pub use uhorse_node_runtime::{
    CommandExecutor, ConnectionConfig, ConnectionState, ExecutionContext, HubConnection, Metrics,
    Node, NodeConfig, NodeError, NodeResult, PermissionManager, PermissionResult, PermissionRule,
    StatusReporter, Workspace, WorkspaceConfig,
};

/// 节点版本
pub const NODE_VERSION: &str = env!("CARGO_PKG_VERSION");
