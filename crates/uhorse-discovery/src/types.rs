//! Core types for service discovery

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Service discovery backend configuration
#[derive(Debug, Clone)]
pub enum Backend {
    /// etcd backend
    Etcd {
        /// etcd endpoints (e.g., ["http://localhost:2379"])
        endpoints: Vec<String>,
        /// Optional username for authentication
        username: Option<String>,
        /// Optional password for authentication
        password: Option<String>,
        /// Optional CA certificate path
        ca_cert_path: Option<String>,
    },

    /// Consul backend
    Consul {
        /// Consul agent address (e.g., "http://localhost:8500")
        address: String,
        /// Optional ACL token
        token: Option<String>,
        /// Optional datacenter
        datacenter: Option<String>,
    },
}

impl Default for Backend {
    fn default() -> Self {
        Backend::Etcd {
            endpoints: vec!["http://localhost:2379".to_string()],
            username: None,
            password: None,
            ca_cert_path: None,
        }
    }
}

/// Service instance metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceMetadata {
    /// Service version
    pub version: Option<String>,
    /// Service weight for load balancing (1-100)
    pub weight: Option<u32>,
    /// Service tags
    pub tags: Vec<String>,
    /// Custom metadata key-value pairs
    pub custom: HashMap<String, String>,
    /// Region/zone for locality-aware routing
    pub zone: Option<String>,
    /// Deployment environment (dev/staging/prod)
    pub environment: Option<String>,
}

/// Service instance representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInstance {
    /// Unique instance identifier
    pub id: String,
    /// Service name
    pub name: String,
    /// IP address or hostname
    pub address: String,
    /// Service port
    pub port: u16,
    /// Instance metadata
    #[serde(default)]
    pub metadata: ServiceMetadata,
}

impl ServiceInstance {
    /// Create a new service instance
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        address: impl Into<String>,
        port: u16,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            address: address.into(),
            port,
            metadata: ServiceMetadata::default(),
        }
    }

    /// Get the service name
    pub fn service_name(&self) -> &str {
        &self.name
    }

    /// Get the full address (address:port)
    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }

    /// Get HTTP URL
    pub fn http_url(&self) -> String {
        format!("http://{}", self.endpoint())
    }

    /// Get HTTPS URL
    pub fn https_url(&self) -> String {
        format!("https://{}", self.endpoint())
    }

    /// Set service version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.metadata.version = Some(version.into());
        self
    }

    /// Set service weight
    pub fn with_weight(mut self, weight: u32) -> Self {
        self.metadata.weight = Some(weight.clamp(1, 100));
        self
    }

    /// Set service zone
    pub fn with_zone(mut self, zone: impl Into<String>) -> Self {
        self.metadata.zone = Some(zone.into());
        self
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.metadata.tags.push(tag.into());
        self
    }

    /// Add custom metadata
    pub fn with_custom(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.custom.insert(key.into(), value.into());
        self
    }
}

/// Service health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Service is healthy and accepting traffic
    Healthy,
    /// Service is unhealthy and should be removed from rotation
    Unhealthy,
    /// Service status is unknown (no recent health check)
    Unknown,
}

impl Default for HealthStatus {
    fn default() -> Self {
        HealthStatus::Unknown
    }
}

/// Health check configuration
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Health check endpoint path
    pub path: String,
    /// Check interval in seconds
    pub interval_secs: u64,
    /// Check timeout in seconds
    pub timeout_secs: u64,
    /// Number of failures before marking unhealthy
    pub failure_threshold: u32,
    /// Number of successes before marking healthy
    pub success_threshold: u32,
    /// TTL for lease-based health checks (seconds)
    pub ttl_secs: Option<u64>,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            path: "/health".to_string(),
            interval_secs: 10,
            timeout_secs: 5,
            failure_threshold: 3,
            success_threshold: 1,
            ttl_secs: Some(30),
        }
    }
}

/// Service registration options
#[derive(Debug, Clone)]
pub struct RegistrationOptions {
    /// TTL for the registration lease (seconds)
    pub ttl_secs: u64,
    /// Enable health checking
    pub health_check: Option<HealthCheckConfig>,
    /// Overwrite existing registration with same ID
    pub overwrite: bool,
}

impl Default for RegistrationOptions {
    fn default() -> Self {
        Self {
            ttl_secs: 30,
            health_check: Some(HealthCheckConfig::default()),
            overwrite: false,
        }
    }
}
