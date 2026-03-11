//! # uHorse Cache
//!
//! 分布式缓存层，提供 Redis 集成、会话缓存、令牌黑名单和缓存策略

pub mod policy;
pub mod redis;
pub mod session;
pub mod token_blacklist;

pub use policy::{CachePolicy, EvictionPolicy};
pub use redis::RedisCache;
pub use session::SessionCache;
pub use token_blacklist::TokenBlacklist;

/// Cache result type
pub type Result<T> = anyhow::Result<T>;
