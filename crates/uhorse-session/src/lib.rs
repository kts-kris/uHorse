//! # uHorse Session
//!
//! 会话层，管理会话生命周期和隔离策略。

pub mod manager;
pub mod store;
pub mod isolation;

pub use manager::SessionManager;
