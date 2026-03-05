//! # uHorse Gateway
//!
//! 网关层，处理 HTTP API 和 WebSocket 连接。

pub mod api;
pub mod auth;
pub mod http;
pub mod middleware;
pub mod store;
pub mod websocket;

pub use store::MemoryStore;
pub use websocket::{ConnectionManager, WsEvent};
