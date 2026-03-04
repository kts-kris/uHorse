//! # API 模块
//!
//! 提供 REST API 的类型定义、路由和处理器。

pub mod types;
pub mod routes;
pub mod handlers;

pub use types::*;
pub use routes::create_api_router;
