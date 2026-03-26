//! # uHorse Node Runtime
//!
//! 本地执行节点运行时，负责接收云端中枢下发的命令并在本地执行。

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod connection;
pub mod error;
pub mod executor;
pub mod node;
pub mod permission;
pub mod status;
pub mod versioning;
pub mod workspace;

pub use connection::{ConnectionConfig, ConnectionState, HubConnection};
pub use error::{NodeError, NodeResult};
pub use executor::{CommandExecutor, ExecutionContext};
pub use node::{Node, NodeConfig};
pub use permission::{PermissionManager, PermissionResult, PermissionRule};
pub use status::{Metrics, StatusReporter};
pub use uhorse_protocol::{NodeId, NotificationEvent, NotificationEventKind};
pub use versioning::{
    CheckpointRecord, DiffTarget, FileChangeKind, RestorePreview, RestoreResult, VersionManager,
    VersionStatusEntry, WorkspaceDiff, WorkspaceVersionStatus,
};
pub use workspace::{Workspace, WorkspaceConfig};

/// 节点版本
pub const NODE_RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");
