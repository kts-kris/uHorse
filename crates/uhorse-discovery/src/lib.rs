//! # uHorse Service Discovery Module
//!
//! 企业级服务发现模块，支持 etcd 和 Consul 两种后端。
//!
//! ## Features
//!
//! - 服务注册与发现
//! - 健康检查 (心跳 + TTL)
//! - 自动故障转移
//! - 负载均衡集成
//!
//! ## Example
//!
//! ```rust,no_run
//! use uhorse_discovery::{DiscoveryClient, ServiceInstance, Backend};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 创建 etcd 客户端
//!     let client = DiscoveryClient::new(Backend::Etcd {
//!         endpoints: vec!["http://localhost:2379".to_string()],
//!         username: None,
//!         password: None,
//!         ca_cert_path: None,
//!     }).await?;
//!
//!     // 注册服务
//!     let instance = ServiceInstance::new(
//!         "uhorse-gateway-1",
//!         "uhorse-gateway",
//!         "192.168.1.10",
//!         8080,
//!     );
//!     client.register(&instance).await?;
//!
//!     // 发现服务
//!     let instances = client.discover("uhorse-gateway").await?;
//!     println!("Found {} instances", instances.len());
//!
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod types;
pub mod registry;
pub mod health;
pub mod failover;

#[cfg(feature = "etcd")]
pub mod etcd;

#[cfg(feature = "consul")]
pub mod consul;

pub use error::{Error, Result};
pub use types::{
    Backend, ServiceInstance, ServiceMetadata,
    HealthCheckConfig, RegistrationOptions, HealthStatus,
};
pub use registry::{DiscoveryClient, ServiceRegistry, InMemoryRegistry};
pub use health::{HealthChecker, HealthCheckResult, HealthTracker};
pub use failover::{
    FailoverConfig, FailoverManager, FailoverRecord, FailoverStatus, FailoverStats, FailoverStrategy,
    FailureRecord, FailureType,
};

// Re-exports for convenience
#[cfg(feature = "etcd")]
pub use etcd::EtcdClient;

#[cfg(feature = "consul")]
pub use consul::ConsulClient;
