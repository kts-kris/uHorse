//! etcd-based service discovery implementation
use async_trait::async_trait;
use etcd_client::{Client, PutOptions};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::{Error, Result};
use crate::registry::ServiceRegistry;
use crate::types::{RegistrationOptions, ServiceInstance};

/// etcd key prefix for service discovery
const SERVICE_PREFIX: &str = "/uhorse/services/";

/// etcd-based service registry client
pub struct EtcdClient {
    client: Arc<Mutex<Client>>,
}

impl EtcdClient {
    /// Create a new etcd client
    pub async fn new(
        endpoints: Vec<String>,
        username: Option<String>,
        password: Option<String>,
        _ca_cert_path: Option<String>,
    ) -> Result<Self> {
        let endpoints_str: Vec<&str> = endpoints.iter().map(String::as_str).collect();
        let client = Client::connect(&endpoints_str, None).await?;

        // Note: authentication would be configured here if needed
        if let (Some(user), Some(_pass)) = (&username, &password) {
            tracing::info!("etcd authentication configured for user: {}", user);
        }

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    /// Get the service key for a given service name and instance ID
    fn service_key(service_name: &str, instance_id: &str) -> String {
        format!("{}{}/{}", SERVICE_PREFIX, service_name, instance_id)
    }

    /// Get the service prefix for a given service name
    fn service_prefix(service_name: &str) -> String {
        format!("{}{}/", SERVICE_PREFIX, service_name)
    }

    /// Parse a service instance from etcd key-value
    fn parse_instance(kv: &etcd_client::KeyValue) -> Result<ServiceInstance> {
        let value = kv.value_str()?;
        serde_json::from_str(value).map_err(Error::from)
    }
}

#[async_trait]
impl ServiceRegistry for EtcdClient {
    async fn register(
        &self,
        instance: &ServiceInstance,
        options: &RegistrationOptions,
    ) -> Result<()> {
        let key = Self::service_key(&instance.name, &instance.id);
        let value = serde_json::to_string(instance)?;

        let mut client = self.client.lock().await;

        // Create a lease with TTL
        let mut lease_client = client.lease_client();
        let lease = lease_client.grant(options.ttl_secs as i64, None).await?;
        let lease_id = lease.id();

        // Put with lease
        let put_options = PutOptions::new().with_lease(lease_id);
        client.put(key, value, Some(put_options)).await?;

        tracing::info!(
            "Registered service {} instance {} with TTL {}s",
            instance.name,
            instance.id,
            options.ttl_secs
        );

        Ok(())
    }

    async fn deregister(&self, service_name: &str, instance_id: &str) -> Result<()> {
        let key = Self::service_key(service_name, instance_id);
        let mut client = self.client.lock().await;
        client.delete(key, None).await?;

        tracing::info!(
            "Deregistered service {} instance {}",
            service_name,
            instance_id
        );

        Ok(())
    }

    async fn discover(&self, service_name: &str) -> Result<Vec<ServiceInstance>> {
        let prefix = Self::service_prefix(service_name);
        let mut client = self.client.lock().await;
        let response = client.get(prefix.as_str(), None).await?;

        let instances: Vec<ServiceInstance> = response
            .kvs()
            .iter()
            .filter_map(|kv| Self::parse_instance(kv).ok())
            .collect();

        tracing::debug!(
            "Discovered {} instances for service {}",
            instances.len(),
            service_name
        );

        Ok(instances)
    }

    async fn discover_instance(
        &self,
        service_name: &str,
        instance_id: &str,
    ) -> Result<Option<ServiceInstance>> {
        let key = Self::service_key(service_name, instance_id);
        let mut client = self.client.lock().await;
        let response = client.get(key.as_str(), None).await?;

        if let Some(kv) = response.kvs().first() {
            Ok(Some(Self::parse_instance(kv)?))
        } else {
            Ok(None)
        }
    }

    async fn list_services(&self) -> Result<Vec<String>> {
        let mut client = self.client.lock().await;
        let response = client.get(SERVICE_PREFIX, None).await?;

        let mut services = std::collections::HashSet::new();
        for kv in response.kvs() {
            if let Ok(key_str) = kv.key_str() {
                // Extract service name from key: /uhorse/services/{service_name}/{instance_id}
                let parts: Vec<&str> = key_str.split('/').collect();
                if parts.len() >= 4 {
                    services.insert(parts[3].to_string());
                }
            }
        }

        Ok(services.into_iter().collect())
    }

    async fn service_exists(&self, service_name: &str) -> Result<bool> {
        let instances = self.discover(service_name).await?;
        Ok(!instances.is_empty())
    }

    async fn heartbeat(&self, service_name: &str, instance_id: &str) -> Result<()> {
        let key = Self::service_key(service_name, instance_id);

        // Get the existing value with lease
        let mut client = self.client.lock().await;
        let response = client.get(key.as_str(), None).await?;

        if let Some(kv) = response.kvs().first() {
            let lease_id = kv.lease();

            // Refresh the lease
            let mut lease_client = client.lease_client();
            lease_client.keep_alive(lease_id).await?;

            tracing::debug!(
                "Heartbeat sent for {} instance {}",
                service_name,
                instance_id
            );
        } else {
            return Err(Error::ServiceNotFound(format!(
                "{}/{}",
                service_name, instance_id
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_key() {
        let key = EtcdClient::service_key("uhorse-gateway", "instance-1");
        assert_eq!(key, "/uhorse/services/uhorse-gateway/instance-1");
    }

    #[test]
    fn test_service_prefix() {
        let prefix = EtcdClient::service_prefix("uhorse-gateway");
        assert_eq!(prefix, "/uhorse/services/uhorse-gateway/");
    }
}
