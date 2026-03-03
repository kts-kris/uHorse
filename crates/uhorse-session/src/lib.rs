//! # uHorse Session
//!
//! 会话层，管理会话生命周期和隔离策略。

pub mod isolation;
pub mod manager;
pub mod store;

pub use manager::SessionManager;
