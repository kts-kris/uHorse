//! Field-level encryption for sensitive data

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt;
use std::sync::Arc;
use tracing::info;
use zeroize::ZeroizeOnDrop;

/// Encryption key (32 bytes for AES-256)
#[derive(Clone, ZeroizeOnDrop)]
pub struct EncryptionKey([u8; 32]);

impl EncryptionKey {
    /// Create a new encryption key from bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Generate a random encryption key
    pub fn generate() -> Self {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        Self(key)
    }

    /// Create from base64-encoded string
    pub fn from_base64(s: &str) -> Result<Self> {
        let bytes = BASE64
            .decode(s)
            .map_err(|e| anyhow!("Failed to decode base64 key: {}", e))?;

        if bytes.len() != 32 {
            return Err(anyhow!(
                "Invalid key length: expected 32 bytes, got {}",
                bytes.len()
            ));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        Ok(Self(key))
    }

    /// Encode to base64
    pub fn to_base64(&self) -> String {
        BASE64.encode(self.0)
    }

    /// Get the underlying bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for EncryptionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EncryptionKey([REDACTED])")
    }
}

/// Key identifier for key rotation
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct KeyId(String);

impl KeyId {
    /// Create a new key ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the key ID string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for KeyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Key version for key rotation
#[derive(Debug, Clone)]
pub struct KeyVersion {
    /// Key ID
    pub id: KeyId,
    /// The encryption key
    pub key: EncryptionKey,
    /// When this key was created (timestamp in seconds)
    pub created_at: u64,
    /// Whether this is the current primary key
    pub is_primary: bool,
}

/// Key manager for handling encryption keys and rotation
pub struct KeyManager {
    /// All key versions
    keys: Vec<KeyVersion>,
    /// Current primary key index
    primary_key_idx: usize,
}

impl KeyManager {
    /// Create a new key manager with a primary key
    pub fn new(primary_key: EncryptionKey) -> Self {
        let key_version = KeyVersion {
            id: KeyId::new(format!("key-{}", chrono::Utc::now().timestamp())),
            key: primary_key,
            created_at: chrono::Utc::now().timestamp() as u64,
            is_primary: true,
        };

        Self {
            keys: vec![key_version],
            primary_key_idx: 0,
        }
    }

    /// Get the current primary key
    pub fn primary_key(&self) -> &EncryptionKey {
        &self.keys[self.primary_key_idx].key
    }

    /// Get the current primary key ID
    pub fn primary_key_id(&self) -> &KeyId {
        &self.keys[self.primary_key_idx].id
    }

    /// Add a new key version
    pub fn add_key(&mut self, key: EncryptionKey) -> KeyId {
        let key_version = KeyVersion {
            id: KeyId::new(format!("key-{}", chrono::Utc::now().timestamp())),
            key,
            created_at: chrono::Utc::now().timestamp() as u64,
            is_primary: false,
        };
        let id = key_version.id.clone();
        self.keys.push(key_version);
        id
    }

    /// Rotate to a new primary key
    pub fn rotate(&mut self, new_key: EncryptionKey) -> KeyId {
        // Mark old primary as non-primary
        self.keys[self.primary_key_idx].is_primary = false;

        // Add new key as primary
        let key_version = KeyVersion {
            id: KeyId::new(format!("key-{}", chrono::Utc::now().timestamp())),
            key: new_key,
            created_at: chrono::Utc::now().timestamp() as u64,
            is_primary: true,
        };
        let id = key_version.id.clone();
        self.keys.push(key_version);
        self.primary_key_idx = self.keys.len() - 1;

        info!("Rotated encryption key to {}", id);
        id
    }

    /// Get a key by ID
    pub fn get_key(&self, id: &KeyId) -> Option<&EncryptionKey> {
        self.keys.iter().find(|k| &k.id == id).map(|k| &k.key)
    }
}

/// Field encryptor for encrypting sensitive data
pub struct FieldEncryptor {
    /// Key manager
    key_manager: Arc<std::sync::RwLock<KeyManager>>,
}

impl FieldEncryptor {
    /// Create a new field encryptor
    pub fn new(key: EncryptionKey) -> Self {
        Self {
            key_manager: Arc::new(std::sync::RwLock::new(KeyManager::new(key))),
        }
    }

    /// Create with existing key manager
    pub fn with_key_manager(key_manager: Arc<std::sync::RwLock<KeyManager>>) -> Self {
        Self { key_manager }
    }

    /// Encrypt a value
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedField> {
        let key_manager = self
            .key_manager
            .read()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        let key = key_manager.primary_key();
        let key_id = key_manager.primary_key_id().clone();

        // Create cipher
        let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
            .map_err(|e| anyhow!("Failed to create cipher: {}", e))?;

        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        Ok(EncryptedField {
            key_id,
            nonce: nonce_bytes.to_vec(),
            ciphertext,
        })
    }

    /// Encrypt a JSON-serializable value
    pub fn encrypt_json<T: Serialize>(&self, value: &T) -> Result<EncryptedField> {
        let json =
            serde_json::to_vec(value).map_err(|e| anyhow!("Failed to serialize value: {}", e))?;
        self.encrypt(&json)
    }

    /// Decrypt a value
    pub fn decrypt(&self, encrypted: &EncryptedField) -> Result<Vec<u8>> {
        let key_manager = self
            .key_manager
            .read()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        let key = key_manager
            .get_key(&encrypted.key_id)
            .ok_or_else(|| anyhow!("Key not found: {}", encrypted.key_id))?;

        // Create cipher
        let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
            .map_err(|e| anyhow!("Failed to create cipher: {}", e))?;

        // Decrypt
        let nonce = Nonce::from_slice(&encrypted.nonce);
        let plaintext = cipher
            .decrypt(nonce, encrypted.ciphertext.as_slice())
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        Ok(plaintext)
    }

    /// Decrypt to a JSON-deserializable value
    pub fn decrypt_json<T: DeserializeOwned>(&self, encrypted: &EncryptedField) -> Result<T> {
        let plaintext = self.decrypt(encrypted)?;
        serde_json::from_slice(&plaintext)
            .map_err(|e| anyhow!("Failed to deserialize value: {}", e))
    }

    /// Rotate encryption key
    pub fn rotate_key(&self, new_key: EncryptionKey) -> Result<KeyId> {
        let mut key_manager = self
            .key_manager
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        Ok(key_manager.rotate(new_key))
    }
}

/// Encrypted field representation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EncryptedField {
    /// Key ID used for encryption
    pub key_id: KeyId,
    /// Nonce (12 bytes for AES-GCM)
    #[serde(with = "serde_base64")]
    pub nonce: Vec<u8>,
    /// Encrypted data
    #[serde(with = "serde_base64")]
    pub ciphertext: Vec<u8>,
}

/// Base64 serialization helper
mod serde_base64 {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(data: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error> {
        BASE64.encode(data).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(deserializer)?;
        BASE64.decode(&s).map_err(D::Error::custom)
    }
}

impl EncryptedField {
    /// Create from encrypted data
    pub fn new(key_id: KeyId, nonce: Vec<u8>, ciphertext: Vec<u8>) -> Self {
        Self {
            key_id,
            nonce,
            ciphertext,
        }
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| anyhow!("Failed to serialize: {}", e))
    }

    /// Deserialize from JSON string
    pub fn from_json(s: &str) -> Result<Self> {
        serde_json::from_str(s).map_err(|e| anyhow!("Failed to deserialize: {}", e))
    }
}

/// Data classification levels
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum DataClassification {
    /// Public data - no encryption required
    Public,
    /// Internal data - encryption optional
    #[default]
    Internal,
    /// Confidential data - encryption required
    Confidential,
    /// Restricted data - encryption + access control required
    Restricted,
}

/// Sensitive field marker trait
pub trait SensitiveField: Serialize + DeserializeOwned {
    /// Get the data classification for this field
    fn classification() -> DataClassification;

    /// Get the field name for audit logging
    fn field_name() -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_key_generation() {
        let key = EncryptionKey::generate();
        let base64 = key.to_base64();
        let decoded = EncryptionKey::from_base64(&base64).unwrap();
        assert_eq!(key.as_bytes(), decoded.as_bytes());
    }

    #[test]
    fn test_field_encryption() {
        let key = EncryptionKey::generate();
        let encryptor = FieldEncryptor::new(key);

        let plaintext = b"Hello, World!";
        let encrypted = encryptor.encrypt(plaintext).unwrap();
        let decrypted = encryptor.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_json_encryption() {
        let key = EncryptionKey::generate();
        let encryptor = FieldEncryptor::new(key);

        let data = serde_json::json!({
            "name": "Alice",
            "email": "alice@example.com"
        });

        let encrypted = encryptor.encrypt_json(&data).unwrap();
        let decrypted: serde_json::Value = encryptor.decrypt_json(&encrypted).unwrap();

        assert_eq!(data, decrypted);
    }

    #[test]
    fn test_key_rotation() {
        let key1 = EncryptionKey::generate();
        let encryptor = FieldEncryptor::new(key1);

        // Encrypt with original key
        let plaintext = b"Secret data";
        let encrypted = encryptor.encrypt(plaintext).unwrap();

        // Rotate key
        let key2 = EncryptionKey::generate();
        encryptor.rotate_key(key2).unwrap();

        // Old data should still be decryptable
        let decrypted = encryptor.decrypt(&encrypted).unwrap();
        assert_eq!(plaintext.to_vec(), decrypted);
    }
}
