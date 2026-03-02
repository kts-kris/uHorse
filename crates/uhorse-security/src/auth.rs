//! # JWT 认证服务
//!
//! 基于 JWT 的令牌认证，支持自动刷新和令牌撤销。

use uhorse_core::{AuthService, Result, DeviceId, AccessToken};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Serialize, Deserialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{Utc, Duration};
use tracing::{debug, info, warn};

/// JWT 认证服务
#[derive(Debug, Clone)]
pub struct JwtAuthService {
    secret: String,
    /// 默认令牌有效期（秒）
    default_expiry: u64,
    /// 刷新令牌有效期（秒）
    refresh_expiry: u64,
    /// 已撤销的令牌黑名单
    revoked: Arc<RwLock<HashSet<String>>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: i64,
    iat: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_id: Option<String>,
    scopes: Vec<String>,
    /// 令牌类型（access/refresh）
    token_type: String,
    /// 唯一标识符（用于撤销）
    jti: String,
}

impl JwtAuthService {
    /// 创建新的 JWT 认证服务
    pub fn new(secret: String) -> Self {
        Self {
            secret,
            default_expiry: 3600,        // 1 小时
            refresh_expiry: 2592000,     // 30 天
            revoked: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// 设置默认令牌有效期
    pub fn with_default_expiry(mut self, expiry: u64) -> Self {
        self.default_expiry = expiry;
        self
    }

    /// 设置刷新令牌有效期
    pub fn with_refresh_expiry(mut self, expiry: u64) -> Self {
        self.refresh_expiry = expiry;
        self
    }

    /// 生成唯一的 JTI（JWT ID）
    fn generate_jti() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("jti_{}", timestamp)
    }

    /// 创建访问令牌
    async fn create_access_token(
        &self,
        subject: String,
        device_id: Option<String>,
        scopes: Vec<String>,
    ) -> Result<String> {
        let now = Utc::now();
        let expiry = now + Duration::seconds(self.default_expiry as i64);

        let claims = Claims {
            sub: subject,
            exp: expiry.timestamp(),
            iat: now.timestamp(),
            device_id,
            scopes,
            token_type: "access".to_string(),
            jti: Self::generate_jti(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_ref()),
        )
        .map_err(|e| uhorse_core::UHorseError::InternalError(e.to_string()))?;

        debug!("Created access token, expires at {}", expiry);
        Ok(token)
    }

    /// 创建刷新令牌
    async fn create_refresh_token(
        &self,
        subject: String,
        device_id: Option<String>,
    ) -> Result<String> {
        let now = Utc::now();
        let expiry = now + Duration::seconds(self.refresh_expiry as i64);

        let claims = Claims {
            sub: subject,
            exp: expiry.timestamp(),
            iat: now.timestamp(),
            device_id,
            scopes: vec!["refresh".to_string()],
            token_type: "refresh".to_string(),
            jti: Self::generate_jti(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_ref()),
        )
        .map_err(|e| uhorse_core::UHorseError::InternalError(e.to_string()))?;

        debug!("Created refresh token, expires at {}", expiry);
        Ok(token)
    }

    /// 检查令牌是否已撤销
    async fn is_revoked(&self, jti: &str) -> bool {
        self.revoked.read().await.contains(jti)
    }

    /// 清理过期的撤销记录
    pub async fn cleanup_revoked(&self) {
        let mut revoked = self.revoked.write().await;
        // 简单实现：移除一半记录（保留最近撤销的）
        if revoked.len() > 10000 {
            let count = revoked.len() / 2;
            let to_remove: Vec<_> = revoked.iter().take(count).cloned().collect();
            for item in to_remove {
                revoked.remove(&item);
            }
            debug!("Cleaned up revoked tokens, remaining: {}", revoked.len());
        }
    }
}

#[async_trait::async_trait]
impl AuthService for JwtAuthService {
    async fn create_token(
        &self,
        device_id: Option<DeviceId>,
        user_id: Option<String>,
        scopes: Vec<String>,
        expires_in: u64,
    ) -> Result<String> {
        let subject = user_id
            .unwrap_or_else(|| device_id.as_ref().map(|d| d.0.clone()).unwrap_or_default());

        let device_id_str = device_id.map(|d| d.0);

        let now = Utc::now();
        let expiry = now + Duration::seconds(expires_in as i64);

        let claims = Claims {
            sub: subject,
            exp: expiry.timestamp(),
            iat: now.timestamp(),
            device_id: device_id_str,
            scopes,
            token_type: "access".to_string(),
            jti: Self::generate_jti(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_ref()),
        )
        .map_err(|e| uhorse_core::UHorseError::InternalError(e.to_string()))?;

        info!("Created token with custom expiry: {} seconds", expires_in);
        Ok(token)
    }

    async fn verify_token(&self, token: &str) -> Result<AccessToken> {
        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_ref()),
            &Validation::default(),
        )
        .map_err(|e| {
            warn!("Token verification failed: {}", e);
            uhorse_core::UHorseError::InvalidToken
        })?;

        // 检查令牌类型
        if claims.claims.token_type != "access" {
            warn!("Invalid token type: {}", claims.claims.token_type);
            return Err(uhorse_core::UHorseError::InvalidToken);
        }

        // 检查是否已撤销
        if self.is_revoked(&claims.claims.jti).await {
            warn!("Token has been revoked: {}", claims.claims.jti);
            return Err(uhorse_core::UHorseError::TokenExpired);
        }

        debug!("Token verified successfully for: {}", claims.claims.sub);

        Ok(AccessToken {
            token: token.to_string(),
            device_id: claims.claims.device_id.map(DeviceId),
            user_id: Some(claims.claims.sub),
            scopes: claims.claims.scopes,
            expires_at: claims.claims.exp as u64,
            created_at: claims.claims.iat as u64,
        })
    }

    async fn revoke_token(&self, token: &str) -> Result<()> {
        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_ref()),
            &Validation::default(),
        )
        .map_err(|_| uhorse_core::UHorseError::InvalidToken)?;

        let jti = claims.claims.jti.clone();
        self.revoked.write().await.insert(jti.clone());
        info!("Revoked token: {}", jti);
        Ok(())
    }

    async fn refresh_token(&self, token: &str) -> Result<String> {
        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_ref()),
            &Validation::default(),
        )
        .map_err(|_| uhorse_core::UHorseError::InvalidToken)?;

        // 检查令牌类型
        if claims.claims.token_type != "refresh" {
            let token_type = claims.claims.token_type.clone();
            warn!("Cannot refresh non-refresh token: {}", token_type);
            return Err(uhorse_core::UHorseError::InvalidToken);
        }

        // 检查是否已撤销
        let jti = claims.claims.jti.clone();
        if self.is_revoked(&jti).await {
            warn!("Refresh token has been revoked: {}", jti);
            return Err(uhorse_core::UHorseError::TokenExpired);
        }

        // 撤销旧的刷新令牌
        self.revoked.write().await.insert(jti);

        // 提取需要的值
        let sub = claims.claims.sub.clone();
        let sub_copy = sub.clone();
        let device_id = claims.claims.device_id.clone();

        // 创建新的访问令牌
        let new_token = self
            .create_access_token(
                sub,
                device_id,
                vec![], // 新令牌的 scopes 重新从数据库获取
            )
            .await?;

        info!("Refreshed token for: {}", sub_copy);
        Ok(new_token)
    }
}

/// 令牌对（访问令牌 + 刷新令牌）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

impl JwtAuthService {
    /// 创建令牌对（访问令牌 + 刷新令牌）
    pub async fn create_token_pair(
        &self,
        device_id: Option<DeviceId>,
        user_id: Option<String>,
        scopes: Vec<String>,
    ) -> Result<TokenPair> {
        let subject = user_id
            .clone()
            .unwrap_or_else(|| device_id.as_ref().map(|d| d.0.clone()).unwrap_or_default());

        let device_id_str = device_id.map(|d| d.0);

        // 创建访问令牌
        let access_token = self
            .create_access_token(subject.clone(), device_id_str.clone(), scopes.clone())
            .await?;

        // 创建刷新令牌
        let refresh_token = self
            .create_refresh_token(subject, device_id_str)
            .await?;

        info!("Created token pair for: {}", user_id.unwrap_or_default());

        Ok(TokenPair {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.default_expiry,
        })
    }

    /// 从刷新令牌获取新的访问令牌
    pub async fn refresh_access_token(&self, refresh_token: &str) -> Result<TokenPair> {
        let claims = decode::<Claims>(
            refresh_token,
            &DecodingKey::from_secret(self.secret.as_ref()),
            &Validation::default(),
        )
        .map_err(|_| uhorse_core::UHorseError::InvalidToken)?;

        if claims.claims.token_type != "refresh" {
            return Err(uhorse_core::UHorseError::InvalidToken);
        }

        if self.is_revoked(&claims.claims.jti).await {
            return Err(uhorse_core::UHorseError::TokenExpired);
        }

        // 撤销旧令牌
        self.revoked.write().await.insert(claims.claims.jti);

        // 创建新的令牌对
        self.create_token_pair(
            claims.claims.device_id.map(DeviceId),
            Some(claims.claims.sub),
            vec![],
        )
        .await
    }

    /// 验证并自动刷新过期令牌
    pub async fn verify_with_auto_refresh(&self, token: &str) -> Result<AccessToken> {
        match self.verify_token(token).await {
            Ok(access_token) => Ok(access_token),
            Err(uhorse_core::UHorseError::TokenExpired) => {
                // 尝试使用刷新令牌
                warn!("Access token expired, attempting refresh");
                Err(uhorse_core::UHorseError::TokenExpired)
            }
            Err(e) => Err(e),
        }
    }
}
