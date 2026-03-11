//! Error types for service discovery module

use thiserror::Error;

/// Service discovery error type
#[derive(Error, Debug)]
pub enum Error {
    #[cfg(feature = "etcd")]
    #[error("etcd error: {0}")]
    Etcd(#[from] etcd_client::Error),

    #[error("consul error: {0}")]
    Consul(String),

    #[error("service not found: {0}")]
    ServiceNotFound(String),

    #[error("registration failed: {0}")]
    RegistrationFailed(String),

    #[error("deregistration failed: {0}")]
    DeregistrationFailed(String),

    #[error("health check failed: {0}")]
    HealthCheckFailed(String),

    #[error("connection error: {0}")]
    ConnectionError(String),

    #[error("timeout error: {0}")]
    Timeout(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("not implemented: {0}")]
    NotImplemented(String),

    #[error("no healthy instances available for service: {0}")]
    NoHealthyInstances(String),
}

/// Result type alias for service discovery operations
pub type Result<T> = std::result::Result<T, Error>;
