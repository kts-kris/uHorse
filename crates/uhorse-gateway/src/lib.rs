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
pub use openapi::{OpenApiInfo, OpenApiManager, ServerInfo, SwaggerUi, SwaggerUiConfig};
pub use ratelimit::{
    DistributedConfig, DistributedRateLimiter, EndpointRateLimitConfig, EndpointRateLimiter,
    GlobalRateLimitConfig, GlobalRateLimiter, RateLimitAlgorithm, RateLimitResult,
    UserRateLimitConfig, UserRateLimiter,
};
pub use store::MemoryStore;
pub use versioning::{
    ApiVersion, CompatibilityChecker, CompatibilityLevel, DeprecationInfo, DeprecationManager,
    VersionParser, VersionedPath,
};
pub use websocket::{ConnectionManager, WsEvent};
