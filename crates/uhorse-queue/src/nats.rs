//! NATS client implementation

use anyhow::{anyhow, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// NATS client configuration
#[derive(Debug, Clone)]
pub struct NatsConfig {
    /// NATS server URL
    pub url: String,
    /// Maximum reconnect attempts
    pub max_reconnect_attempts: u32,
    /// Reconnect delay
    pub reconnect_delay: Duration,
    /// Request timeout
    pub request_timeout: Duration,
}

impl Default for NatsConfig {
    fn default() -> Self {
        Self {
            url: "nats://127.0.0.1:4222".to_string(),
            max_reconnect_attempts: 10,
            reconnect_delay: Duration::from_secs(1),
            request_timeout: Duration::from_secs(30),
        }
    }
}

impl NatsConfig {
    /// Create a new configuration
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Default::default()
        }
    }
}

/// NATS client wrapper
pub struct NatsClient {
    /// NATS connection
    connection: Option<async_nats::Client>,
    /// Configuration
    config: NatsConfig,
    /// Connection state
    connected: Arc<RwLock<bool>>,
}

impl NatsClient {
    /// Create a new NATS client
    pub fn new(config: NatsConfig) -> Self {
        Self {
            connection: None,
            config,
            connected: Arc::new(RwLock::new(false)),
        }
    }

    /// Connect to NATS server
    pub async fn connect(&mut self) -> Result<()> {
        let client = async_nats::connect(&self.config.url)
            .await
            .map_err(|e| anyhow!("Failed to connect to NATS: {}", e))?;

        self.connection = Some(client);

        let mut connected = self.connected.write().await;
        *connected = true;

        info!("Connected to NATS server at {}", self.config.url);
        Ok(())
    }

    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        let connected = self.connected.read().await;
        *connected
    }

    /// Publish a message
    pub async fn publish(&self, subject: &str, payload: Vec<u8>) -> Result<()> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to NATS"))?;

        connection
            .publish(subject.to_string(), payload.into())
            .await
            .map_err(|e| anyhow!("Failed to publish message: {}", e))?;

        debug!("Published message to subject: {}", subject);
        Ok(())
    }

    /// Subscribe to a subject
    pub async fn subscribe(&self, subject: &str) -> Result<async_nats::Subscriber> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to NATS"))?;

        let subscriber = connection
            .subscribe(subject.to_string())
            .await
            .map_err(|e| anyhow!("Failed to subscribe: {}", e))?;

        info!("Subscribed to subject: {}", subject);
        Ok(subscriber)
    }

    /// Request-reply pattern
    pub async fn request(
        &self,
        subject: &str,
        payload: Vec<u8>,
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to NATS"))?;

        let result = tokio::time::timeout(
            timeout,
            connection.request(subject.to_string(), payload.into()),
        )
        .await
        .map_err(|_| anyhow!("Request timeout"))?
        .map_err(|e| anyhow!("Request failed: {}", e))?;

        Ok(Some(result.payload.to_vec()))
    }

    /// Disconnect from NATS
    pub async fn disconnect(&mut self) -> Result<()> {
        self.connection = None;

        let mut connected = self.connected.write().await;
        *connected = false;

        info!("Disconnected from NATS server");
        Ok(())
    }
}

impl std::fmt::Debug for NatsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NatsClient")
            .field("config", &self.config)
            .field("connected", &self.connected)
            .finish()
    }
}
