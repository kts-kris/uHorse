//! Consul-based service discovery implementation
use async_trait::async_trait;

use crate::error::{Error, Result};
use crate::registry::ServiceRegistry;
use crate::types::{RegistrationOptions, ServiceInstance};

/// Consul-based service registry client
pub struct ConsulClient {
    address: String,
    token: Option<String>,
    datacenter: Option<String>,
    #[cfg(feature = "health-check")]
    client: reqwest::Client,
}

impl ConsulClient {
    /// Create a new Consul client
    pub fn new(address: String, token: Option<String>, datacenter: Option<String>) -> Result<Self> {
        Ok(Self {
            address,
            token,
            datacenter,
            #[cfg(feature = "health-check")]
            client: reqwest::Client::new(),
        })
    }

    /// Build the Consul API URL
    fn api_url(&self, path: &str) -> String {
        format!("{}{}", self.address, path)
    }

    /// Add authentication headers to a request builder
    #[cfg(feature = "health-check")]
    fn add_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let mut builder = builder;
        if let Some(ref token) = self.token {
            builder = builder.header("X-Consul-Token", token);
        }
        if let Some(ref dc) = self.datacenter {
            builder = builder.query(&[("dc", dc)]);
        }
        builder
    }
}

#[async_trait]
impl ServiceRegistry for ConsulClient {
    async fn register(
        &self,
        instance: &ServiceInstance,
        options: &RegistrationOptions,
    ) -> Result<()> {
        #[cfg(feature = "health-check")]
        {
            let url = self.api_url("/v1/agent/service/register");

            let health_check = options.health_check.as_ref();

            let body = serde_json::json!({
                "ID": instance.id,
                "Name": instance.name,
                "Address": instance.address,
                "Port": instance.port,
                "Check": health_check.map(|hc| {
                    serde_json::json!({
                        "HTTP": format!("{}{}", instance.http_url(), hc.path),
                        "Interval": format!("{}s", hc.interval_secs),
                        "Timeout": format!("{}s", hc.timeout_secs),
                    })
                }),
                "Meta": &instance.metadata.custom,
                "Tags": &instance.metadata.tags,
            });

            let response = self
                .add_auth(self.client.put(&url).json(&body))
                .send()
                .await
                .map_err(|e| Error::Consul(e.to_string()))?;

            if !response.status().is_success() {
                return Err(Error::RegistrationFailed(format!(
                    "Consul registration failed: {}",
                    response.status()
                )));
            }

            tracing::info!(
                "Registered service {} instance {} in Consul",
                instance.name,
                instance.id
            );
        }

        #[cfg(not(feature = "health-check"))]
        {
            tracing::warn!("Consul registration requires health-check feature");
            return Err(Error::Config(
                "health-check feature not enabled".to_string(),
            ));
        }

        Ok(())
    }

    async fn deregister(&self, service_name: &str, instance_id: &str) -> Result<()> {
        #[cfg(feature = "health-check")]
        {
            let url = self.api_url(&format!("/v1/agent/service/deregister/{}", instance_id));

            let response = self
                .add_auth(self.client.put(&url))
                .send()
                .await
                .map_err(|e| Error::Consul(e.to_string()))?;

            if !response.status().is_success() {
                return Err(Error::DeregistrationFailed(format!(
                    "Consul deregistration failed: {}",
                    response.status()
                )));
            }

            tracing::info!(
                "Deregistered service {} instance {} from Consul",
                service_name,
                instance_id
            );
        }

        #[cfg(not(feature = "health-check"))]
        {
            let _ = (service_name, instance_id);
            tracing::warn!("Consul deregistration requires health-check feature");
        }

        Ok(())
    }

    async fn discover(&self, service_name: &str) -> Result<Vec<ServiceInstance>> {
        #[cfg(feature = "health-check")]
        {
            let url = self.api_url(&format!("/v1/catalog/service/{}", service_name));

            let response = self
                .add_auth(self.client.get(&url))
                .send()
                .await
                .map_err(|e| Error::Consul(e.to_string()))?;

            if !response.status().is_success() {
                return Err(Error::ServiceNotFound(format!(
                    "Consul discovery failed: {}",
                    response.status()
                )));
            }

            let services: Vec<ConsulService> = response
                .json()
                .await
                .map_err(|e| Error::Consul(e.to_string()))?;

            let instances: Vec<ServiceInstance> = services
                .into_iter()
                .map(|s| ServiceInstance {
                    id: s.ServiceID,
                    name: s.ServiceName,
                    address: s.ServiceAddress.unwrap_or_else(|| s.Address),
                    port: s.ServicePort as u16,
                    metadata: crate::types::ServiceMetadata {
                        tags: s.ServiceTags,
                        custom: s.ServiceMeta,
                        ..Default::default()
                    },
                })
                .collect();

            tracing::debug!(
                "Discovered {} instances for service {} from Consul",
                instances.len(),
                service_name
            );

            return Ok(instances);
        }

        #[cfg(not(feature = "health-check"))]
        {
            let _ = service_name;
            Ok(vec![])
        }
    }

    async fn discover_instance(
        &self,
        service_name: &str,
        instance_id: &str,
    ) -> Result<Option<ServiceInstance>> {
        let instances = self.discover(service_name).await?;
        Ok(instances.into_iter().find(|i| i.id == instance_id))
    }

    async fn list_services(&self) -> Result<Vec<String>> {
        #[cfg(feature = "health-check")]
        {
            let url = self.api_url("/v1/catalog/services");

            let response = self
                .add_auth(self.client.get(&url))
                .send()
                .await
                .map_err(|e| Error::Consul(e.to_string()))?;

            if !response.status().is_success() {
                return Err(Error::Consul(format!(
                    "Failed to list services: {}",
                    response.status()
                )));
            }

            let services: std::collections::HashMap<String, Vec<String>> = response
                .json()
                .await
                .map_err(|e| Error::Consul(e.to_string()))?;

            Ok(services.into_keys().collect())
        }

        #[cfg(not(feature = "health-check"))]
        {
            Ok(vec![])
        }
    }

    async fn service_exists(&self, service_name: &str) -> Result<bool> {
        let instances = self.discover(service_name).await?;
        Ok(!instances.is_empty())
    }

    async fn heartbeat(&self, _service_name: &str, _instance_id: &str) -> Result<()> {
        // Consul uses its own health check mechanism, no explicit heartbeat needed
        tracing::debug!("Consul heartbeat is handled by agent health checks");
        Ok(())
    }
}

/// Consul service response structure
#[cfg(feature = "health-check")]
#[derive(Debug, serde::Deserialize)]
struct ConsulService {
    #[serde(rename = "ServiceID")]
    ServiceID: String,
    #[serde(rename = "ServiceName")]
    ServiceName: String,
    Address: String,
    #[serde(rename = "ServiceAddress")]
    ServiceAddress: Option<String>,
    #[serde(rename = "ServicePort")]
    ServicePort: i32,
    #[serde(rename = "ServiceTags", default)]
    ServiceTags: Vec<String>,
    #[serde(rename = "ServiceMeta", default)]
    ServiceMeta: std::collections::HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_url() {
        let client = ConsulClient::new("http://localhost:8500".to_string(), None, None).unwrap();
        assert_eq!(
            client.api_url("/v1/catalog/services"),
            "http://localhost:8500/v1/catalog/services"
        );
    }
}
