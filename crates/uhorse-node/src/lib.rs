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
//! use uhorse_node::{Node, NodeConfig, Workspace};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 创建工作空间
//!     let workspace = Workspace::new("/Users/xxx/projects")?;
//!
//!     // 创建节点配置
//!     let config = NodeConfig {
//!         hub_url: "wss://hub.uhorse.ai".to_string(),
//!         workspace,
//!         ..Default::default()
//!     };
//!
//!     // 启动节点
//!     let node = Node::new(config);
//!     node.start().await?;
//!
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod error;
pub mod workspace;
pub mod permission;
pub mod executor;
pub mod connection;
pub mod status;
pub mod node;
pub mod tools;

pub use error::{NodeError, NodeResult};
pub use workspace::{Workspace, WorkspaceConfig};
pub use permission::{PermissionManager, PermissionRule, PermissionResult};
pub use executor::{CommandExecutor, ExecutionContext};
pub use connection::{HubConnection, ConnectionConfig, ConnectionState};
pub use status::{StatusReporter, Metrics};
pub use node::{Node, NodeConfig};
pub use tools::{DatabaseExecutor, BrowserExecutor, SkillExecutor};

/// 节点版本
pub const NODE_VERSION: &str = env!("CARGO_PKG_VERSION");
