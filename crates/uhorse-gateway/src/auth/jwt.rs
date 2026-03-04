//! # JWT Utilities
//!
//! JWT 令牌生成与验证。

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// JWT 配置
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// 签名密钥
    pub secret: String,
    /// 访问令牌过期时间（秒）
    pub access_token_expiry: u64,
    /// 刷新令牌过期时间（秒）
    pub refresh_token_expiry: u64,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: "uhorse-default-secret-change-in-production".to_string(),
            access_token_expiry: 86400,   // 24 小时
            refresh_token_expiry: 604800, // 7 天
        }
    }
}

/// JWT Claims
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// 主题（用户 ID）
    pub sub: String,
    /// 用户名
    pub username: String,
    /// 角色
    pub role: String,
    /// 令牌类型：access / refresh
    pub token_type: String,
    /// 签发时间
    pub iat: u64,
    /// 过期时间
    pub exp: u64,
}

impl Claims {
    /// 创建新的 Claims
    pub fn new(
        user_id: impl Into<String>,
        username: impl Into<String>,
        role: impl Into<String>,
        token_type: impl Into<String>,
        expiry_secs: u64,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            sub: user_id.into(),
            username: username.into(),
            role: role.into(),
            token_type: token_type.into(),
            iat: now,
            exp: now + expiry_secs,
        }
    }
}

/// JWT 服务
pub struct JwtService {
    config: JwtConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl fmt::Debug for JwtService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JwtService")
            .field("config", &self.config)
            .field("encoding_key", &"<EncodingKey>")
            .field("decoding_key", &"<DecodingKey>")
            .finish()
    }
}

impl JwtService {
    /// 创建新的 JWT 服务
    pub fn new(config: JwtConfig) -> Self {
        let encoding_key = EncodingKey::from_secret(config.secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(config.secret.as_bytes());

        Self {
            config,
            encoding_key,
            decoding_key,
        }
    }

    /// 生成访问令牌
    pub fn generate_access_token(
        &self,
        user_id: &str,
        username: &str,
        role: &str,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let claims = Claims::new(
            user_id,
            username,
            role,
            "access",
            self.config.access_token_expiry,
        );
        encode(&Header::default(), &claims, &self.encoding_key)
    }

    /// 生成刷新令牌
    pub fn generate_refresh_token(
        &self,
        user_id: &str,
        username: &str,
        role: &str,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let claims = Claims::new(
            user_id,
            username,
            role,
            "refresh",
            self.config.refresh_token_expiry,
        );
        encode(&Header::default(), &claims, &self.encoding_key)
    }

    /// 验证令牌
    pub fn verify_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let token_data = decode::<Claims>(token, &self.decoding_key, &Validation::default());
        token_data.map(|data| data.claims)
    }

    /// 获取访问令牌过期时间（秒）
    pub fn access_token_expiry(&self) -> u64 {
        self.config.access_token_expiry
    }
}

impl Default for JwtService {
    fn default() -> Self {
        Self::new(JwtConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_verify_access_token() {
        let service = JwtService::default();
        let token = service
            .generate_access_token("user-1", "admin", "admin")
            .unwrap();
        let claims = service.verify_token(&token).unwrap();

        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.username, "admin");
        assert_eq!(claims.role, "admin");
        assert_eq!(claims.token_type, "access");
    }

    #[test]
    fn test_generate_and_verify_refresh_token() {
        let service = JwtService::default();
        let token = service
            .generate_refresh_token("user-1", "admin", "admin")
            .unwrap();
        let claims = service.verify_token(&token).unwrap();

        assert_eq!(claims.token_type, "refresh");
        assert_eq!(claims.sub, "user-1");
    }

    #[test]
    fn test_verify_invalid_token() {
        let service = JwtService::default();
        let result = service.verify_token("invalid-token");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_token_with_wrong_secret() {
        let service1 = JwtService::new(JwtConfig {
            secret: "secret1".to_string(),
            ..Default::default()
        });
        let service2 = JwtService::new(JwtConfig {
            secret: "secret2".to_string(),
            ..Default::default()
        });

        let token = service1
            .generate_access_token("user-1", "admin", "admin")
            .unwrap();
        let result = service2.verify_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_config() {
        let config = JwtConfig {
            secret: "my-custom-secret".to_string(),
            access_token_expiry: 3600,
            refresh_token_expiry: 86400,
        };
        let service = JwtService::new(config);

        let token = service
            .generate_access_token("user-1", "test", "user")
            .unwrap();
        let claims = service.verify_token(&token).unwrap();

        assert_eq!(claims.sub, "user-1");
        assert_eq!(service.access_token_expiry(), 3600);
    }

    #[test]
    fn test_claims_creation() {
        let claims = Claims::new("user-123", "testuser", "admin", "access", 3600);

        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.role, "admin");
        assert_eq!(claims.token_type, "access");
        // 验证 exp > iat
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_different_users_generate_different_tokens() {
        let service = JwtService::default();

        let token1 = service
            .generate_access_token("user-1", "alice", "user")
            .unwrap();
        let token2 = service
            .generate_access_token("user-2", "bob", "user")
            .unwrap();

        assert_ne!(token1, token2);

        let claims1 = service.verify_token(&token1).unwrap();
        let claims2 = service.verify_token(&token2).unwrap();

        assert_ne!(claims1.sub, claims2.sub);
    }
}
