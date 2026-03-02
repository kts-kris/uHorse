//! # uHorse Gateway
//!
//! 网关层，处理 HTTP API 和 WebSocket 连接。

pub mod websocket;
pub mod http;
pub mod middleware;

pub use websocket::WebSocketHandler;
