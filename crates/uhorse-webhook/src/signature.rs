//! Signature Verification
//!
//! 实现 HMAC-SHA256 签名验证

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// 签名配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningConfig {
    /// 签名密钥
    pub secret: String,
    /// 签名算法
    pub algorithm: SignatureAlgorithm,
    /// 签名头名称
    pub header_name: String,
    /// 签名前缀
    pub prefix: Option<String>,
    /// 时间戳有效期 (秒)
    pub timestamp_tolerance_secs: Option<u64>,
}

impl SigningConfig {
    /// 创建新的签名配置
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            algorithm: SignatureAlgorithm::HmacSha256,
            header_name: "X-Signature".to_string(),
            prefix: Some("sha256=".to_string()),
            timestamp_tolerance_secs: Some(300),
        }
    }

    /// 不使用前缀
    pub fn without_prefix(mut self) -> Self {
        self.prefix = None;
        self
    }

    /// 设置自定义头名称
    pub fn with_header(mut self, header: impl Into<String>) -> Self {
        self.header_name = header.into();
        self
    }
}

/// 签名算法
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureAlgorithm {
    /// HMAC-SHA256
    HmacSha256,
    /// HMAC-SHA512
    HmacSha512,
}

/// 签名验证器
pub struct SignatureVerifier {
    /// 配置
    config: SigningConfig,
}

impl SignatureVerifier {
    /// 创建新的验证器
    pub fn new(config: SigningConfig) -> Self {
        Self { config }
    }

    /// 计算签名
    pub fn sign(&self, payload: &[u8]) -> String {
        match self.config.algorithm {
            SignatureAlgorithm::HmacSha256 => self.sign_hmac_sha256(payload),
            SignatureAlgorithm::HmacSha512 => self.sign_hmac_sha512(payload),
        }
    }

    /// HMAC-SHA256 签名
    fn sign_hmac_sha256(&self, payload: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(self.config.secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(payload);
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }

    /// HMAC-SHA512 签名
    fn sign_hmac_sha512(&self, payload: &[u8]) -> String {
        type HmacSha512 = Hmac<sha2::Sha512>;
        let mut mac = HmacSha512::new_from_slice(self.config.secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(payload);
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }

    /// 生成签名头值
    pub fn generate_signature_header(&self, payload: &[u8]) -> String {
        let signature = self.sign(payload);

        if let Some(ref prefix) = self.config.prefix {
            format!("{}{}", prefix, signature)
        } else {
            signature
        }
    }

    /// 验证签名
    pub fn verify(&self, payload: &[u8], signature: &str) -> bool {
        let expected = self.sign(payload);

        // 移除前缀（如果有）
        let signature = if let Some(ref prefix) = self.config.prefix {
            if signature.starts_with(prefix) {
                &signature[prefix.len()..]
            } else {
                signature
            }
        } else {
            signature
        };

        // 常量时间比较
        self.constant_time_compare(expected.as_bytes(), signature.as_bytes())
    }

    /// 验证签名头
    pub fn verify_header(&self, payload: &[u8], header_value: &str) -> bool {
        self.verify(payload, header_value)
    }

    /// 常量时间比较
    fn constant_time_compare(&self, a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }

        let mut result = 0u8;
        for (x, y) in a.iter().zip(b.iter()) {
            result |= x ^ y;
        }

        result == 0
    }

    /// 创建带时间戳的签名
    pub fn sign_with_timestamp(&self, payload: &[u8], timestamp: i64) -> String {
        let timestamped_payload = format!("{}:{}", timestamp, String::from_utf8_lossy(payload));
        self.sign(timestamped_payload.as_bytes())
    }

    /// 验证带时间戳的签名
    pub fn verify_with_timestamp(
        &self,
        payload: &[u8],
        signature: &str,
        timestamp: i64,
    ) -> Result<bool, String> {
        // 检查时间戳是否过期
        if let Some(tolerance) = self.config.timestamp_tolerance_secs {
            let now = chrono::Utc::now().timestamp();
            let diff = (now - timestamp).abs() as u64;

            if diff > tolerance {
                return Err(format!("Timestamp expired: {} seconds old", diff));
            }
        }

        let expected = self.sign_with_timestamp(payload, timestamp);
        Ok(self.constant_time_compare(expected.as_bytes(), signature.as_bytes()))
    }
}

/// 创建签名中间件头
pub fn create_signature_headers(config: &SigningConfig, payload: &[u8]) -> Vec<(String, String)> {
    let verifier = SignatureVerifier::new(config.clone());

    let mut headers = Vec::new();

    // 根据是否配置时间戳选择签名方式
    if config.timestamp_tolerance_secs.is_some() {
        let timestamp = chrono::Utc::now().timestamp();
        let signature = verifier.sign_with_timestamp(payload, timestamp);

        let header_value = if let Some(ref prefix) = config.prefix {
            format!("{}{}", prefix, signature)
        } else {
            signature
        };

        headers.push((config.header_name.clone(), header_value));
        headers.push(("X-Signature-Timestamp".to_string(), timestamp.to_string()));
    } else {
        let signature = verifier.generate_signature_header(payload);
        headers.push((config.header_name.clone(), signature));
    }

    headers
}

/// 验证请求签名
pub fn verify_request_signature(
    config: &SigningConfig,
    payload: &[u8],
    headers: &std::collections::HashMap<String, String>,
) -> Result<bool, String> {
    let verifier = SignatureVerifier::new(config.clone());

    // 获取签名头
    let signature = headers
        .get(&config.header_name)
        .ok_or_else(|| "Missing signature header".to_string())?;

    // 移除前缀（如果有）
    let signature = if let Some(ref prefix) = config.prefix {
        if signature.starts_with(prefix) {
            &signature[prefix.len()..]
        } else {
            signature.as_str()
        }
    } else {
        signature.as_str()
    };

    // 检查时间戳（如果配置了）
    if let Some(_tolerance) = config.timestamp_tolerance_secs {
        let timestamp_str = headers
            .get("X-Signature-Timestamp")
            .ok_or_else(|| "Missing timestamp header".to_string())?;

        let timestamp: i64 = timestamp_str
            .parse()
            .map_err(|_| "Invalid timestamp".to_string())?;

        return verifier.verify_with_timestamp(payload, signature, timestamp);
    }

    Ok(verifier.verify(payload, signature))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_sign_and_verify() {
        let config = SigningConfig::new("my-secret-key");
        let verifier = SignatureVerifier::new(config);

        let payload = b"test payload";

        let signature = verifier.sign(payload);
        assert!(!signature.is_empty());

        // 验证签名
        assert!(verifier.verify(payload, &signature));

        // 错误的签名
        assert!(!verifier.verify(payload, "wrong-signature"));

        // 错误的 payload
        assert!(!verifier.verify(b"wrong payload", &signature));
    }

    #[test]
    fn test_signature_header() {
        let config = SigningConfig::new("my-secret-key");
        let verifier = SignatureVerifier::new(config);

        let payload = b"test payload";
        let header_value = verifier.generate_signature_header(payload);

        assert!(header_value.starts_with("sha256="));
        assert!(verifier.verify_header(payload, &header_value));
    }

    #[test]
    fn test_signature_without_prefix() {
        let config = SigningConfig::new("my-secret-key").without_prefix();
        let verifier = SignatureVerifier::new(config);

        let payload = b"test payload";
        let signature = verifier.sign(payload);

        assert!(!signature.starts_with("sha256="));
        assert!(verifier.verify(payload, &signature));
    }

    #[test]
    fn test_timestamp_signature() {
        let config = SigningConfig::new("my-secret-key");
        let verifier = SignatureVerifier::new(config);

        let payload = b"test payload";
        let timestamp = chrono::Utc::now().timestamp();

        let signature = verifier.sign_with_timestamp(payload, timestamp);

        let result = verifier.verify_with_timestamp(payload, &signature, timestamp);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_expired_timestamp() {
        let config = SigningConfig::new("my-secret-key");
        let verifier = SignatureVerifier::new(config);

        let payload = b"test payload";
        // 10 分钟前
        let timestamp = chrono::Utc::now().timestamp() - 600;

        let signature = verifier.sign_with_timestamp(payload, timestamp);

        let result = verifier.verify_with_timestamp(payload, &signature, timestamp);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_and_verify_headers() {
        let config = SigningConfig::new("my-secret-key");
        let payload = b"test payload";

        let headers = create_signature_headers(&config, payload);
        let header_map: HashMap<String, String> = headers.into_iter().collect();

        let result = verify_request_signature(&config, payload, &header_map);
        match result {
            Ok(valid) => assert!(valid, "Signature verification returned false"),
            Err(e) => panic!("Signature verification error: {}", e),
        }
    }
}
