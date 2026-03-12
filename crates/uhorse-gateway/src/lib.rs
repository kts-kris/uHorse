//! # uHorse Gateway
//!
//! 网关层，处理 HTTP API 和 WebSocket 连接。

pub mod api;
pub mod auth;
pub mod http;
pub mod lb;
pub mod middleware;
pub mod openapi;
pub mod ratelimit;
pub mod store;
pub mod versioning;
pub mod websocket;

pub use lb::{
    HealthAwareLoadBalancer, InstanceStats, LeastConnectionLoadBalancer, LoadBalanceStrategy,
    LoadBalancer, LoadBalancerFactory, RoundRobinLoadBalancer, WeightedLoadBalancer,
};
pub use openapi::{
    OpenApiInfo, OpenApiManager, ServerInfo, SwaggerUi, SwaggerUiConfig,
};
pub use ratelimit::{
    GlobalRateLimiter, GlobalRateLimitConfig, UserRateLimiter, UserRateLimitConfig,
    EndpointRateLimiter, EndpointRateLimitConfig, DistributedRateLimiter, DistributedConfig,
    RateLimitResult, RateLimitAlgorithm,
};
pub use store::MemoryStore;
pub use versioning::{
    ApiVersion, VersionedPath, VersionParser, DeprecationManager, DeprecationInfo,
    CompatibilityChecker, CompatibilityLevel,
};
pub use websocket::{ConnectionManager, WsEvent};
