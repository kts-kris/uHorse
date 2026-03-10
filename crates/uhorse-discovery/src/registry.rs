//! Service registry trait and client implementation
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::Result;
use crate::types::{Backend, RegistrationOptions, ServiceInstance};

#[cfg(feature = "etcd")]
use crate::etcd::EtcdClient;

#[cfg(feature = "consul")]
use crate::consul::ConsulClient;

/// Service registry trait - defines the interface for service registration and discovery
#[async_trait]
pub trait ServiceRegistry: Send + Sync {
    /// Register a service instance
    async fn register(&self, instance: &ServiceInstance, options: &RegistrationOptions) -> Result<()>;

    /// Deregister a service instance
    async fn deregister(&self, service_name: &str, instance_id: &str) -> Result<()>;

    /// Discover all instances of a service
    async fn discover(&self, service_name: &str) -> Result<Vec<ServiceInstance>>;

    /// Discover a specific instance
    async fn discover_instance(&self, service_name: &str, instance_id: &str) -> Result<Option<ServiceInstance>>;

    /// List all registered services
    async fn list_services(&self) -> Result<Vec<String>>;

    /// Check if a service exists
    async fn service_exists(&self, service_name: &str) -> Result<bool>;

    /// Heartbeat / keep-alive for a registered service
    async fn heartbeat(&self, service_name: &str, instance_id: &str) -> Result<()>;
}

/// Discovery client - unified interface for service discovery
pub struct DiscoveryClient {
    registry: Arc<dyn ServiceRegistry>,
}

impl DiscoveryClient {
    /// Create a new discovery client with the specified backend
    pub async fn new(backend: Backend) -> Result<Self> {
        let registry: Arc<dyn ServiceRegistry> = match backend {
            #[cfg(feature = "etcd")]
            Backend::Etcd { endpoints, username, password, ca_cert_path } => {
                Arc::new(EtcdClient::new(endpoints, username, password, ca_cert_path).await?)
            }
            #[cfg(not(feature = "etcd"))]
            Backend::Etcd { .. } => {
                return Err(crate::error::Error::Config("etcd feature not enabled".to_string()));
            }

            #[cfg(feature = "consul")]
            Backend::Consul { address, token, datacenter } => {
                Arc::new(ConsulClient::new(address, token, datacenter)?)
            }
            #[cfg(not(feature = "consul"))]
            Backend::Consul { .. } => {
                return Err(crate::error::Error::Config("consul feature not enabled".to_string()));
            }
        };

        Ok(Self { registry })
    }

    /// Create a new discovery client from an existing registry
    pub fn from_registry(registry: Arc<dyn ServiceRegistry>) -> Self {
        Self { registry }
    }

    /// Create a new in-memory discovery client
    pub fn in_memory() -> Self {
        Self {
            registry: Arc::new(InMemoryRegistry::new()),
        }
    }

    /// Register a service instance with default options
    pub async fn register(&self, instance: &ServiceInstance) -> Result<()> {
        self.registry.register(instance, &RegistrationOptions::default()).await
    }

    /// Register a service instance with custom options
    pub async fn register_with_options(&self, instance: &ServiceInstance, options: &RegistrationOptions) -> Result<()> {
        self.registry.register(instance, options).await
    }

    /// Deregister a service instance
    pub async fn deregister(&self, service_name: &str, instance_id: &str) -> Result<()> {
        self.registry.deregister(service_name, instance_id).await
    }

    /// Discover all instances of a service
    pub async fn discover(&self, service_name: &str) -> Result<Vec<ServiceInstance>> {
        self.registry.discover(service_name).await
    }

    /// Discover a specific instance
    pub async fn discover_instance(&self, service_name: &str, instance_id: &str) -> Result<Option<ServiceInstance>> {
        self.registry.discover_instance(service_name, instance_id).await
    }

    /// List all registered services
    pub async fn list_services(&self) -> Result<Vec<String>> {
        self.registry.list_services().await
    }

    /// Check if a service exists
    pub async fn service_exists(&self, service_name: &str) -> Result<bool> {
        self.registry.service_exists(service_name).await
    }

    /// Send heartbeat for a registered service
    pub async fn heartbeat(&self, service_name: &str, instance_id: &str) -> Result<()> {
        self.registry.heartbeat(service_name, instance_id).await
    }

    /// Get the underlying registry
    pub fn registry(&self) -> Arc<dyn ServiceRegistry> {
        self.registry.clone()
    }
}

/// In-memory service registry for testing and single-node deployments
pub struct InMemoryRegistry {
    services: Arc<RwLock<HashMap<String, Vec<ServiceInstance>>>>,
}

impl InMemoryRegistry {
    /// Create a new in-memory registry
    pub fn new() -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ServiceRegistry for InMemoryRegistry {
    async fn register(&self, instance: &ServiceInstance, _options: &RegistrationOptions) -> Result<()> {
        let mut services = self.services.write().await;
        let instances = services.entry(instance.name.clone()).or_insert_with(Vec::new);

        // Check for existing instance with same ID
        if let Some(existing) = instances.iter_mut().find(|i| i.id == instance.id) {
            *existing = instance.clone();
        } else {
            instances.push(instance.clone());
        }

        Ok(())
    }

    async fn deregister(&self, service_name: &str, instance_id: &str) -> Result<()> {
        let mut services = self.services.write().await;
        if let Some(instances) = services.get_mut(service_name) {
            instances.retain(|i| i.id != instance_id);
            if instances.is_empty() {
                services.remove(service_name);
            }
        }
        Ok(())
    }

    async fn discover(&self, service_name: &str) -> Result<Vec<ServiceInstance>> {
        let services = self.services.read().await;
        Ok(services.get(service_name).cloned().unwrap_or_default())
    }

    async fn discover_instance(&self, service_name: &str, instance_id: &str) -> Result<Option<ServiceInstance>> {
        let services = self.services.read().await;
        Ok(services
            .get(service_name)
            .and_then(|instances| instances.iter().find(|i| i.id == instance_id).cloned()))
    }

    async fn list_services(&self) -> Result<Vec<String>> {
        let services = self.services.read().await;
        Ok(services.keys().cloned().collect())
    }

    async fn service_exists(&self, service_name: &str) -> Result<bool> {
        let services = self.services.read().await;
        Ok(services.contains_key(service_name))
    }

    async fn heartbeat(&self, _service_name: &str, _instance_id: &str) -> Result<()> {
        // No-op for in-memory registry
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_registry() {
        let registry = InMemoryRegistry::new();

        let instance = ServiceInstance::new("test-1", "test-service", "127.0.0.1", 8080);
        registry.register(&instance, &RegistrationOptions::default()).await.unwrap();

        let instances = registry.discover("test-service").await.unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].id, "test-1");

        registry.deregister("test-service", "test-1").await.unwrap();
        let instances = registry.discover("test-service").await.unwrap();
        assert!(instances.is_empty());
    }
}
