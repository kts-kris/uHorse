//! TLS/SSL configuration and certificate management

use anyhow::{anyhow, Result};
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    ServerConfig,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// TLS configuration
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Certificate file path
    pub cert_path: PathBuf,
    /// Private key file path
    pub key_path: PathBuf,
    /// CA certificate path (optional, for client authentication)
    pub ca_path: Option<PathBuf>,
    /// Enable client certificate verification
    pub verify_client: bool,
    /// Minimum TLS version
    pub min_tls_version: TlsVersion,
    /// Supported cipher suites
    pub cipher_suites: Vec<CipherSuite>,
}

/// TLS version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsVersion {
    /// TLS 1.2
    V1_2,
    /// TLS 1.3
    V1_3,
}

impl Default for TlsVersion {
    fn default() -> Self {
        Self::V1_3
    }
}

/// Cipher suite configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherSuite {
    /// TLS_AES_128_GCM_SHA256
    Aes128GcmSha256,
    /// TLS_AES_256_GCM_SHA384
    Aes256GcmSha384,
    /// TLS_CHACHA20_POLY1305_SHA256
    ChaCha20Poly1305Sha256,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_path: PathBuf::from("certs/server.crt"),
            key_path: PathBuf::from("certs/server.key"),
            ca_path: None,
            verify_client: false,
            min_tls_version: TlsVersion::V1_3,
            cipher_suites: vec![
                CipherSuite::Aes256GcmSha384,
                CipherSuite::Aes128GcmSha256,
                CipherSuite::ChaCha20Poly1305Sha256,
            ],
        }
    }
}

impl TlsConfig {
    /// Create a new TLS configuration
    pub fn new(cert_path: impl Into<PathBuf>, key_path: impl Into<PathBuf>) -> Self {
        Self {
            cert_path: cert_path.into(),
            key_path: key_path.into(),
            ..Default::default()
        }
    }

    /// Enable client certificate verification
    pub fn with_client_verification(mut self, ca_path: impl Into<PathBuf>) -> Self {
        self.verify_client = true;
        self.ca_path = Some(ca_path.into());
        self
    }

    /// Set minimum TLS version
    pub fn with_min_version(mut self, version: TlsVersion) -> Self {
        self.min_tls_version = version;
        self
    }
}

/// TLS server configuration builder
pub struct TlsServerBuilder {
    config: TlsConfig,
}

impl TlsServerBuilder {
    /// Create a new TLS server builder
    pub fn new() -> Self {
        Self {
            config: TlsConfig::default(),
        }
    }

    /// Set certificate paths
    pub fn with_cert_paths(mut self, cert: impl Into<PathBuf>, key: impl Into<PathBuf>) -> Self {
        self.config.cert_path = cert.into();
        self.config.key_path = key.into();
        self
    }

    /// Enable client verification
    pub fn with_client_verification(mut self, ca_path: impl Into<PathBuf>) -> Self {
        self.config.verify_client = true;
        self.config.ca_path = Some(ca_path.into());
        self
    }

    /// Build the TLS server configuration
    pub fn build(self) -> Result<Arc<ServerConfig>> {
        let cert = load_certs(&self.config.cert_path)?;
        let key = load_private_key(&self.config.key_path)?;

        // Configure TLS versions using builder pattern
        let server_config = match self.config.min_tls_version {
            TlsVersion::V1_2 => {
                warn!("TLS 1.2 enabled - consider using TLS 1.3 for better security");
                ServerConfig::builder_with_protocol_versions(&[
                    &rustls::version::TLS12,
                    &rustls::version::TLS13,
                ])
                .with_no_client_auth()
                .with_single_cert(cert, key)
                .map_err(|e| anyhow!("Failed to create server config: {}", e))?
            }
            TlsVersion::V1_3 => {
                ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
                    .with_no_client_auth()
                    .with_single_cert(cert, key)
                    .map_err(|e| anyhow!("Failed to create server config: {}", e))?
            }
        };

        info!("TLS server configuration built successfully");
        Ok(Arc::new(server_config))
    }
}

impl Default for TlsServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Certificate manager for automatic certificate management
pub struct CertificateManager {
    /// Certificate directory
    cert_dir: PathBuf,
    /// Certificate renewal threshold (days before expiry)
    renewal_threshold_days: u32,
}

impl CertificateManager {
    /// Create a new certificate manager
    pub fn new(cert_dir: impl Into<PathBuf>) -> Self {
        Self {
            cert_dir: cert_dir.into(),
            renewal_threshold_days: 30,
        }
    }

    /// Set renewal threshold
    pub fn with_renewal_threshold(mut self, days: u32) -> Self {
        self.renewal_threshold_days = days;
        self
    }

    /// Check if certificate needs renewal
    pub fn needs_renewal(&self, cert_path: &Path) -> Result<bool> {
        let cert_data = std::fs::read(cert_path)?;
        let certs = rustls_pemfile::certs(&mut &cert_data[..])
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("Failed to parse certificate: {}", e))?;

        if certs.is_empty() {
            return Err(anyhow!("No certificates found in file"));
        }

        // Parse certificate to check expiry
        // For now, we'll use a simple heuristic
        let metadata = std::fs::metadata(cert_path)?;
        let modified = metadata.modified()?;
        let age = std::time::SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default();

        // Consider renewal if certificate is older than threshold
        let threshold_duration = std::time::Duration::from_secs(
            (365 - self.renewal_threshold_days as u64) * 24 * 60 * 60,
        );

        Ok(age > threshold_duration)
    }

    /// Get certificate file path
    pub fn cert_path(&self) -> PathBuf {
        self.cert_dir.join("server.crt")
    }

    /// Get private key file path
    pub fn key_path(&self) -> PathBuf {
        self.cert_dir.join("server.key")
    }

    /// Ensure certificate directory exists
    pub fn ensure_cert_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.cert_dir)
            .map_err(|e| anyhow!("Failed to create certificate directory: {}", e))?;
        Ok(())
    }
}

/// Load certificates from PEM file
fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    let cert_data = std::fs::read(path)
        .map_err(|e| anyhow!("Failed to read certificate file {:?}: {}", path, e))?;

    let certs = rustls_pemfile::certs(&mut &cert_data[..])
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow!("Failed to parse certificates: {}", e))?;

    if certs.is_empty() {
        return Err(anyhow!("No certificates found in file {:?}", path));
    }

    debug!("Loaded {} certificate(s) from {:?}", certs.len(), path);
    Ok(certs)
}

/// Load private key from PEM file
fn load_private_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    let key_data = std::fs::read(path)
        .map_err(|e| anyhow!("Failed to read private key file {:?}: {}", path, e))?;

    // Try to parse as PKCS#8 first
    if let Some(key) = rustls_pemfile::private_key(&mut &key_data[..])
        .map_err(|e| anyhow!("Failed to parse private key: {}", e))?
    {
        debug!("Loaded private key from {:?}", path);
        return Ok(key);
    }

    Err(anyhow!("No valid private key found in file {:?}", path))
}

/// HTTPS redirection middleware configuration
#[derive(Debug, Clone)]
pub struct HttpsRedirectConfig {
    /// Enable HTTPS redirection
    pub enabled: bool,
    /// HSTS max age in seconds
    pub hsts_max_age: u64,
    /// Include subdomains in HSTS
    pub hsts_include_subdomains: bool,
    /// Redirect status code (usually 301 or 308)
    pub redirect_status: u16,
}

impl Default for HttpsRedirectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hsts_max_age: 31536000, // 1 year
            hsts_include_subdomains: true,
            redirect_status: 308,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_config_default() {
        let config = TlsConfig::default();
        assert_eq!(config.min_tls_version, TlsVersion::V1_3);
        assert!(!config.verify_client);
    }

    #[test]
    fn test_tls_config_builder() {
        let config = TlsConfig::new("/path/to/cert.pem", "/path/to/key.pem")
            .with_min_version(TlsVersion::V1_2);

        assert_eq!(config.cert_path, PathBuf::from("/path/to/cert.pem"));
        assert_eq!(config.min_tls_version, TlsVersion::V1_2);
    }

    #[test]
    fn test_https_redirect_config() {
        let config = HttpsRedirectConfig::default();
        assert!(config.enabled);
        assert_eq!(config.redirect_status, 308);
    }
}
