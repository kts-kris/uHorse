//! OpenID Connect (OIDC) Client
//!
//! 实现 OIDC 身份发现和用户信息获取

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::info;

/// OIDC 配置
#[derive(Debug, Clone)]
pub struct OidcConfig {
    /// Issuer URL
    pub issuer: String,
    /// 授权端点
    pub authorization_endpoint: String,
    /// 令牌端点
    pub token_endpoint: String,
    /// 用户信息端点
    pub userinfo_endpoint: String,
    /// JWKS 端点
    pub jwks_uri: String,
    /// 响应类型
    pub response_types_supported: Vec<String>,
    /// 作用域
    pub scopes_supported: Vec<String>,
    /// 客户端 ID
    pub client_id: String,
    /// 客户端密钥
    pub client_secret: String,
    /// 重定向 URI
    pub redirect_uri: String,
}

impl OidcConfig {
    /// 从发现文档创建配置
    pub fn from_discovery(discovery: &OidcDiscovery, client_id: impl Into<String>, client_secret: impl Into<String>, redirect_uri: impl Into<String>) -> Self {
        Self {
            issuer: discovery.issuer.clone(),
            authorization_endpoint: discovery.authorization_endpoint.clone(),
            token_endpoint: discovery.token_endpoint.clone(),
            userinfo_endpoint: discovery.userinfo_endpoint.clone().unwrap_or_default(),
            jwks_uri: discovery.jwks_uri.clone(),
            response_types_supported: discovery.response_types_supported.clone(),
            scopes_supported: discovery.scopes_supported.clone(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            redirect_uri: redirect_uri.into(),
        }
    }

    /// 生成授权 URL
    pub fn authorization_url(&self, state: &str, nonce: &str) -> String {
        let scope = "openid profile email";
        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&nonce={}",
            self.authorization_endpoint,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(scope),
            urlencoding::encode(state),
            urlencoding::encode(nonce),
        )
    }
}

/// OIDC 发现文档
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcDiscovery {
    /// Issuer
    pub issuer: String,
    /// 授权端点
    pub authorization_endpoint: String,
    /// 令牌端点
    pub token_endpoint: String,
    /// 用户信息端点
    pub userinfo_endpoint: Option<String>,
    /// JWKS 端点
    pub jwks_uri: String,
    /// 响应类型
    pub response_types_supported: Vec<String>,
    /// 响应模式
    pub response_modes_supported: Option<Vec<String>>,
    /// 授权类型
    pub grant_types_supported: Option<Vec<String>>,
    /// 主题类型
    pub subject_types_supported: Vec<String>,
    /// 签名算法
    pub id_token_signing_alg_values_supported: Vec<String>,
    /// 作用域
    pub scopes_supported: Vec<String>,
    /// 声明
    pub claims_supported: Option<Vec<String>>,
    /// 端点认证方法
    pub token_endpoint_auth_methods_supported: Option<Vec<String>>,
    /// 代码挑战方法
    pub code_challenge_methods_supported: Option<Vec<String>>,
}

/// OIDC 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// 主题标识符
    pub sub: String,
    /// 用户名
    #[serde(default)]
    pub name: Option<String>,
    /// 名
    #[serde(default)]
    pub given_name: Option<String>,
    /// 姓
    #[serde(default)]
    pub family_name: Option<String>,
    /// 中间名
    #[serde(default)]
    pub middle_name: Option<String>,
    /// 昵称
    #[serde(default)]
    pub nickname: Option<String>,
    /// 首选用户名
    #[serde(default)]
    pub preferred_username: Option<String>,
    /// 个人资料 URL
    #[serde(default)]
    pub profile: Option<String>,
    /// 头像 URL
    #[serde(default)]
    pub picture: Option<String>,
    /// 网站
    #[serde(default)]
    pub website: Option<String>,
    /// 邮箱
    #[serde(default)]
    pub email: Option<String>,
    /// 邮箱已验证
    #[serde(default)]
    pub email_verified: Option<bool>,
    /// 性别
    #[serde(default)]
    pub gender: Option<String>,
    /// 生日
    #[serde(default)]
    pub birthdate: Option<String>,
    /// 时区
    #[serde(default)]
    pub zoneinfo: Option<String>,
    /// 语言
    #[serde(default)]
    pub locale: Option<String>,
    /// 电话号码
    #[serde(default)]
    pub phone_number: Option<String>,
    /// 电话已验证
    #[serde(default)]
    pub phone_number_verified: Option<bool>,
    /// 地址
    #[serde(default)]
    pub address: Option<Address>,
    /// 更新时间
    #[serde(default)]
    pub updated_at: Option<i64>,
}

/// 地址信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    /// 格式化地址
    #[serde(default)]
    pub formatted: Option<String>,
    /// 街道地址
    #[serde(default)]
    pub street_address: Option<String>,
    /// 地区
    #[serde(default)]
    pub locality: Option<String>,
    /// 区域
    #[serde(default)]
    pub region: Option<String>,
    /// 邮编
    #[serde(default)]
    pub postal_code: Option<String>,
    /// 国家
    #[serde(default)]
    pub country: Option<String>,
}

/// ID 令牌载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdTokenPayload {
    /// Issuer
    pub iss: String,
    /// 主题
    pub sub: String,
    /// 受众
    pub aud: String,
    /// 过期时间
    pub exp: i64,
    /// 签发时间
    pub iat: i64,
    /// 认证时间
    #[serde(default)]
    pub auth_time: Option<i64>,
    /// Nonce
    #[serde(default)]
    pub nonce: Option<String>,
    /// 邮箱
    #[serde(default)]
    pub email: Option<String>,
    /// 邮箱已验证
    #[serde(default)]
    pub email_verified: Option<bool>,
    /// 姓名
    #[serde(default)]
    pub name: Option<String>,
}

impl IdTokenPayload {
    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.exp
    }

    /// 验证受众
    pub fn verify_audience(&self, client_id: &str) -> bool {
        self.aud == client_id
    }

    /// 验证签发者
    pub fn verify_issuer(&self, issuer: &str) -> bool {
        self.iss == issuer
    }
}

/// OIDC 客户端
pub struct OidcClient {
    /// 配置
    config: OidcConfig,
    /// HTTP 客户端
    http_client: reqwest::Client,
}

impl OidcClient {
    /// 创建新的 OIDC 客户端
    pub fn new(config: OidcConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }

    /// 发现 OIDC 配置
    pub async fn discover(issuer_url: &str) -> crate::Result<OidcDiscovery> {
        let discovery_url = format!("{}/.well-known/openid-configuration", issuer_url);

        info!("Discovering OIDC configuration from: {}", discovery_url);

        let client = reqwest::Client::new();
        let response = client
            .get(&discovery_url)
            .send()
            .await
            .map_err(|e| crate::SsoError::OidcError(format!("Discovery request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::SsoError::OidcError(format!(
                "Discovery request failed with status: {}",
                response.status()
            )));
        }

        let discovery: OidcDiscovery = response
            .json()
            .await
            .map_err(|e| crate::SsoError::OidcError(format!("Failed to parse discovery document: {}", e)))?;

        Ok(discovery)
    }

    /// 获取授权 URL
    pub fn get_authorization_url(&self, state: &str, nonce: &str) -> String {
        self.config.authorization_url(state, nonce)
    }

    /// 交换授权码获取令牌
    pub async fn exchange_code(&self, code: &str) -> crate::Result<OidcTokenResponse> {
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &self.config.redirect_uri),
            ("client_id", &self.config.client_id),
            ("client_secret", &self.config.client_secret),
        ];

        let response = self
            .http_client
            .post(&self.config.token_endpoint)
            .form(&params)
            .send()
            .await
            .map_err(|e| crate::SsoError::OidcError(format!("Token request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::SsoError::OidcError(format!(
                "Token request failed: {}",
                error_text
            )));
        }

        let token_response: OidcTokenResponse = response
            .json()
            .await
            .map_err(|e| crate::SsoError::OidcError(format!("Failed to parse token response: {}", e)))?;

        Ok(token_response)
    }

    /// 获取用户信息
    pub async fn get_userinfo(&self, access_token: &str) -> crate::Result<UserInfo> {
        let response = self
            .http_client
            .get(&self.config.userinfo_endpoint)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| crate::SsoError::OidcError(format!("UserInfo request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::SsoError::OidcError(format!(
                "UserInfo request failed: {}",
                error_text
            )));
        }

        let user_info: UserInfo = response
            .json()
            .await
            .map_err(|e| crate::SsoError::OidcError(format!("Failed to parse user info: {}", e)))?;

        Ok(user_info)
    }

    /// 解析 ID 令牌
    pub fn parse_id_token(&self, id_token: &str) -> crate::Result<IdTokenPayload> {
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() != 3 {
            return Err(crate::SsoError::OidcError("Invalid ID token format".to_string()));
        }

        let payload_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            parts[1],
        ).map_err(|e| crate::SsoError::OidcError(format!("Failed to decode ID token: {}", e)))?;

        let payload: IdTokenPayload = serde_json::from_slice(&payload_bytes)
            .map_err(|e| crate::SsoError::OidcError(format!("Failed to parse ID token payload: {}", e)))?;

        Ok(payload)
    }

    /// 验证 ID 令牌
    pub fn verify_id_token(&self, id_token: &IdTokenPayload, nonce: Option<&str>) -> crate::Result<bool> {
        // 验证签发者
        if !id_token.verify_issuer(&self.config.issuer) {
            return Err(crate::SsoError::OidcError("Invalid issuer".to_string()));
        }

        // 验证受众
        if !id_token.verify_audience(&self.config.client_id) {
            return Err(crate::SsoError::OidcError("Invalid audience".to_string()));
        }

        // 验证过期时间
        if id_token.is_expired() {
            return Err(crate::SsoError::OidcError("ID token expired".to_string()));
        }

        // 验证 nonce
        if let Some(expected_nonce) = nonce {
            if id_token.nonce.as_deref() != Some(expected_nonce) {
                return Err(crate::SsoError::OidcError("Invalid nonce".to_string()));
            }
        }

        Ok(true)
    }

    /// 刷新令牌
    pub async fn refresh_token(&self, refresh_token: &str) -> crate::Result<OidcTokenResponse> {
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &self.config.client_id),
            ("client_secret", &self.config.client_secret),
        ];

        let response = self
            .http_client
            .post(&self.config.token_endpoint)
            .form(&params)
            .send()
            .await
            .map_err(|e| crate::SsoError::OidcError(format!("Token refresh failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::SsoError::OidcError(format!(
                "Token refresh failed: {}",
                error_text
            )));
        }

        let token_response: OidcTokenResponse = response
            .json()
            .await
            .map_err(|e| crate::SsoError::OidcError(format!("Failed to parse token response: {}", e)))?;

        Ok(token_response)
    }
}

/// OIDC 令牌响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcTokenResponse {
    /// 访问令牌
    pub access_token: String,
    /// 令牌类型
    pub token_type: String,
    /// 过期时间 (秒)
    pub expires_in: Option<i64>,
    /// 刷新令牌
    pub refresh_token: Option<String>,
    /// ID 令牌
    pub id_token: Option<String>,
    /// 作用域
    pub scope: Option<String>,
}

/// URL 编码模块
mod urlencoding {
    pub fn encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oidc_config_authorization_url() {
        let discovery = OidcDiscovery {
            issuer: "https://accounts.example.com".to_string(),
            authorization_endpoint: "https://accounts.example.com/oauth2/auth".to_string(),
            token_endpoint: "https://accounts.example.com/oauth2/token".to_string(),
            userinfo_endpoint: Some("https://accounts.example.com/oauth2/userinfo".to_string()),
            jwks_uri: "https://accounts.example.com/.well-known/jwks.json".to_string(),
            response_types_supported: vec!["code".to_string()],
            response_modes_supported: None,
            grant_types_supported: None,
            subject_types_supported: vec!["public".to_string()],
            id_token_signing_alg_values_supported: vec!["RS256".to_string()],
            scopes_supported: vec!["openid".to_string(), "profile".to_string(), "email".to_string()],
            claims_supported: None,
            token_endpoint_auth_methods_supported: None,
            code_challenge_methods_supported: None,
        };

        let config = OidcConfig::from_discovery(
            &discovery,
            "client-123",
            "secret-456",
            "https://myapp.com/callback",
        );

        let url = config.authorization_url("state-abc", "nonce-xyz");
        assert!(url.contains("client_id=client-123"));
        assert!(url.contains("state=state-abc"));
        assert!(url.contains("nonce=nonce-xyz"));
    }

    #[test]
    fn test_id_token_payload() {
        let payload = IdTokenPayload {
            iss: "https://accounts.example.com".to_string(),
            sub: "user-123".to_string(),
            aud: "client-123".to_string(),
            exp: Utc::now().timestamp() + 3600,
            iat: Utc::now().timestamp(),
            auth_time: None,
            nonce: Some("nonce-xyz".to_string()),
            email: Some("user@example.com".to_string()),
            email_verified: Some(true),
            name: Some("Test User".to_string()),
        };

        assert!(!payload.is_expired());
        assert!(payload.verify_audience("client-123"));
        assert!(!payload.verify_audience("wrong-client"));
        assert!(payload.verify_issuer("https://accounts.example.com"));
    }
}
