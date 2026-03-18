//! Identity Provider (IdP) Integration
//!
//! 支持主流企业 IdP 集成 (Okta/Auth0/Azure AD/Google Workspace)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// IdP 类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IdpType {
    /// Okta
    Okta,
    /// Auth0
    Auth0,
    /// Azure Active Directory
    AzureAd,
    /// Google Workspace
    GoogleWorkspace,
    /// OneLogin
    OneLogin,
    /// 自定义
    Custom,
}

impl std::fmt::Display for IdpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdpType::Okta => write!(f, "okta"),
            IdpType::Auth0 => write!(f, "auth0"),
            IdpType::AzureAd => write!(f, "azure_ad"),
            IdpType::GoogleWorkspace => write!(f, "google_workspace"),
            IdpType::OneLogin => write!(f, "onelogin"),
            IdpType::Custom => write!(f, "custom"),
        }
    }
}

/// IdP 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdpConfig {
    /// IdP 类型
    pub idp_type: IdpType,
    /// 客户端 ID
    pub client_id: String,
    /// 客户端密钥
    pub client_secret: String,
    /// Issuer URL
    pub issuer_url: String,
    /// 授权端点 (可选)
    pub authorization_endpoint: Option<String>,
    /// 令牌端点 (可选)
    pub token_endpoint: Option<String>,
    /// 用户信息端点 (可选)
    pub userinfo_endpoint: Option<String>,
    /// JWKS 端点 (可选)
    pub jwks_uri: Option<String>,
    /// 重定向 URI
    pub redirect_uri: String,
    /// 作用域
    pub scopes: Vec<String>,
    /// 额外参数
    #[serde(default)]
    pub extra_params: HashMap<String, String>,
}

impl IdpConfig {
    /// 创建 Okta 配置
    pub fn okta(domain: &str, client_id: &str, client_secret: &str, redirect_uri: &str) -> Self {
        Self {
            idp_type: IdpType::Okta,
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            issuer_url: format!("https://{}", domain),
            authorization_endpoint: Some(format!("https://{}/oauth2/v1/authorize", domain)),
            token_endpoint: Some(format!("https://{}/oauth2/v1/token", domain)),
            userinfo_endpoint: Some(format!("https://{}/oauth2/v1/userinfo", domain)),
            jwks_uri: Some(format!("https://{}/oauth2/v1/keys", domain)),
            redirect_uri: redirect_uri.to_string(),
            scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            extra_params: HashMap::new(),
        }
    }

    /// 创建 Auth0 配置
    pub fn auth0(domain: &str, client_id: &str, client_secret: &str, redirect_uri: &str) -> Self {
        Self {
            idp_type: IdpType::Auth0,
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            issuer_url: format!("https://{}", domain),
            authorization_endpoint: Some(format!("https://{}/authorize", domain)),
            token_endpoint: Some(format!("https://{}/oauth/token", domain)),
            userinfo_endpoint: Some(format!("https://{}/userinfo", domain)),
            jwks_uri: Some(format!("https://{}/.well-known/jwks.json", domain)),
            redirect_uri: redirect_uri.to_string(),
            scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            extra_params: HashMap::new(),
        }
    }

    /// 创建 Azure AD 配置
    pub fn azure_ad(
        tenant_id: &str,
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
    ) -> Self {
        Self {
            idp_type: IdpType::AzureAd,
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            issuer_url: format!("https://login.microsoftonline.com/{}/v2.0", tenant_id),
            authorization_endpoint: Some(format!(
                "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize",
                tenant_id
            )),
            token_endpoint: Some(format!(
                "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
                tenant_id
            )),
            userinfo_endpoint: Some("https://graph.microsoft.com/oidc/userinfo".to_string()),
            jwks_uri: Some(format!(
                "https://login.microsoftonline.com/{}/discovery/v2.0/keys",
                tenant_id
            )),
            redirect_uri: redirect_uri.to_string(),
            scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            extra_params: HashMap::new(),
        }
    }

    /// 创建 Google Workspace 配置
    pub fn google_workspace(client_id: &str, client_secret: &str, redirect_uri: &str) -> Self {
        Self {
            idp_type: IdpType::GoogleWorkspace,
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            issuer_url: "https://accounts.google.com".to_string(),
            authorization_endpoint: Some(
                "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            ),
            token_endpoint: Some("https://oauth2.googleapis.com/token".to_string()),
            userinfo_endpoint: Some("https://openidconnect.googleapis.com/v1/userinfo".to_string()),
            jwks_uri: Some("https://www.googleapis.com/oauth2/v3/certs".to_string()),
            redirect_uri: redirect_uri.to_string(),
            scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            extra_params: HashMap::new(),
        }
    }

    /// 获取授权 URL
    pub fn authorization_url(&self, state: &str, nonce: &str) -> String {
        let endpoint = self.authorization_endpoint.as_ref().unwrap();
        let scope = self.scopes.join(" ");

        let mut url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&nonce={}",
            endpoint,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(&scope),
            urlencoding::encode(state),
            urlencoding::encode(nonce),
        );

        // 添加额外参数
        for (key, value) in &self.extra_params {
            url.push_str(&format!(
                "&{}={}",
                urlencoding::encode(key),
                urlencoding::encode(value)
            ));
        }

        url
    }
}

/// IdP 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdpUserInfo {
    /// 用户 ID
    pub sub: String,
    /// 邮箱
    pub email: Option<String>,
    /// 邮箱已验证
    pub email_verified: Option<bool>,
    /// 姓名
    pub name: Option<String>,
    /// 名
    pub given_name: Option<String>,
    /// 姓
    pub family_name: Option<String>,
    /// 头像
    pub picture: Option<String>,
    /// 语言
    pub locale: Option<String>,
    /// 组织 ID (企业 IdP)
    pub organization_id: Option<String>,
    /// 部门
    pub department: Option<String>,
    /// 职位
    pub title: Option<String>,
    /// 群组
    pub groups: Option<Vec<String>>,
    /// 角色
    pub roles: Option<Vec<String>>,
    /// 自定义属性
    #[serde(default)]
    pub custom_attributes: HashMap<String, serde_json::Value>,
}

/// IdP 客户端
pub struct IdpClient {
    /// 配置
    config: IdpConfig,
    /// HTTP 客户端
    http_client: reqwest::Client,
}

impl IdpClient {
    /// 创建新的 IdP 客户端
    pub fn new(config: IdpConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }

    /// 获取授权 URL
    pub fn get_authorization_url(&self, state: &str, nonce: &str) -> String {
        self.config.authorization_url(state, nonce)
    }

    /// 交换授权码获取令牌
    pub async fn exchange_code(&self, code: &str) -> crate::Result<IdpTokenResponse> {
        let endpoint = self.config.token_endpoint.as_ref().unwrap();

        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &self.config.redirect_uri),
            ("client_id", &self.config.client_id),
            ("client_secret", &self.config.client_secret),
        ];

        info!(
            "Exchanging authorization code with {}",
            self.config.idp_type
        );

        let response = self
            .http_client
            .post(endpoint)
            .form(&params)
            .send()
            .await
            .map_err(|e| crate::SsoError::IdpError(format!("Token request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::SsoError::IdpError(format!(
                "Token request failed: {}",
                error_text
            )));
        }

        let token_response: IdpTokenResponse = response.json().await.map_err(|e| {
            crate::SsoError::IdpError(format!("Failed to parse token response: {}", e))
        })?;

        Ok(token_response)
    }

    /// 获取用户信息
    pub async fn get_userinfo(&self, access_token: &str) -> crate::Result<IdpUserInfo> {
        let endpoint = self.config.userinfo_endpoint.as_ref().unwrap();

        let response = self
            .http_client
            .get(endpoint)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| crate::SsoError::IdpError(format!("UserInfo request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::SsoError::IdpError(format!(
                "UserInfo request failed: {}",
                error_text
            )));
        }

        // 先获取原始 JSON
        let raw_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| crate::SsoError::IdpError(format!("Failed to parse user info: {}", e)))?;

        // 转换为 IdpUserInfo
        let user_info = self.parse_userinfo(&raw_json);

        Ok(user_info)
    }

    /// 解析用户信息
    fn parse_userinfo(&self, raw: &serde_json::Value) -> IdpUserInfo {
        let mut custom_attributes = HashMap::new();

        // 提取标准字段
        let sub = raw["sub"].as_str().unwrap_or_default().to_string();
        let email = raw["email"].as_str().map(|s| s.to_string());
        let email_verified = raw["email_verified"].as_bool();
        let name = raw["name"].as_str().map(|s| s.to_string());
        let given_name = raw["given_name"].as_str().map(|s| s.to_string());
        let family_name = raw["family_name"].as_str().map(|s| s.to_string());
        let picture = raw["picture"].as_str().map(|s| s.to_string());
        let locale = raw["locale"].as_str().map(|s| s.to_string());

        // 提取企业特定字段
        let organization_id = raw["organization_id"]
            .as_str()
            .or_else(|| raw["tenant_id"].as_str())
            .map(|s| s.to_string());

        let department = raw["department"]
            .as_str()
            .or_else(|| raw["dept"].as_str())
            .map(|s| s.to_string());

        let title = raw["title"]
            .as_str()
            .or_else(|| raw["job_title"].as_str())
            .map(|s| s.to_string());

        let groups = raw["groups"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        });

        let roles = raw["roles"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        });

        // 收集非标准字段为自定义属性
        if let serde_json::Value::Object(map) = raw {
            for (key, value) in map {
                if !matches!(
                    key.as_str(),
                    "sub"
                        | "email"
                        | "email_verified"
                        | "name"
                        | "given_name"
                        | "family_name"
                        | "picture"
                        | "locale"
                        | "organization_id"
                        | "tenant_id"
                        | "department"
                        | "dept"
                        | "title"
                        | "job_title"
                        | "groups"
                        | "roles"
                ) {
                    custom_attributes.insert(key.clone(), value.clone());
                }
            }
        }

        IdpUserInfo {
            sub,
            email,
            email_verified,
            name,
            given_name,
            family_name,
            picture,
            locale,
            organization_id,
            department,
            title,
            groups,
            roles,
            custom_attributes,
        }
    }

    /// 刷新令牌
    pub async fn refresh_token(&self, refresh_token: &str) -> crate::Result<IdpTokenResponse> {
        let endpoint = self.config.token_endpoint.as_ref().unwrap();

        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &self.config.client_id),
            ("client_secret", &self.config.client_secret),
        ];

        let response = self
            .http_client
            .post(endpoint)
            .form(&params)
            .send()
            .await
            .map_err(|e| crate::SsoError::IdpError(format!("Token refresh failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::SsoError::IdpError(format!(
                "Token refresh failed: {}",
                error_text
            )));
        }

        let token_response: IdpTokenResponse = response.json().await.map_err(|e| {
            crate::SsoError::IdpError(format!("Failed to parse token response: {}", e))
        })?;

        Ok(token_response)
    }

    /// 撤销令牌
    pub async fn revoke_token(&self, token: &str) -> crate::Result<bool> {
        // 不是所有 IdP 都支持令牌撤销
        let revoke_url = match self.config.idp_type {
            IdpType::Okta => format!("{}/oauth2/v1/revoke", self.config.issuer_url),
            IdpType::Auth0 => format!("{}/oauth/revoke", self.config.issuer_url),
            IdpType::GoogleWorkspace => "https://oauth2.googleapis.com/revoke".to_string(),
            _ => return Ok(false), // 不支持撤销
        };

        let params = [
            ("token", token),
            ("client_id", &self.config.client_id),
            ("client_secret", &self.config.client_secret),
        ];

        let response = self
            .http_client
            .post(&revoke_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| crate::SsoError::IdpError(format!("Token revocation failed: {}", e)))?;

        Ok(response.status().is_success())
    }

    /// 获取 IdP 类型
    pub fn idp_type(&self) -> &IdpType {
        &self.config.idp_type
    }
}

/// IdP 令牌响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdpTokenResponse {
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
    /// 令牌类型 (Azure AD)
    pub token_type_ext: Option<String>,
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
    fn test_idp_type_display() {
        assert_eq!(IdpType::Okta.to_string(), "okta");
        assert_eq!(IdpType::Auth0.to_string(), "auth0");
        assert_eq!(IdpType::AzureAd.to_string(), "azure_ad");
    }

    #[test]
    fn test_okta_config() {
        let config = IdpConfig::okta(
            "example.okta.com",
            "client-123",
            "secret-456",
            "https://myapp.com/callback",
        );

        assert_eq!(config.idp_type, IdpType::Okta);
        assert_eq!(config.issuer_url, "https://example.okta.com");
        assert!(config.authorization_endpoint.is_some());
        assert!(config.scopes.contains(&"openid".to_string()));
    }

    #[test]
    fn test_auth0_config() {
        let config = IdpConfig::auth0(
            "example.auth0.com",
            "client-123",
            "secret-456",
            "https://myapp.com/callback",
        );

        assert_eq!(config.idp_type, IdpType::Auth0);
        assert!(config.token_endpoint.unwrap().contains("/oauth/token"));
    }

    #[test]
    fn test_azure_ad_config() {
        let config = IdpConfig::azure_ad(
            "tenant-id-123",
            "client-123",
            "secret-456",
            "https://myapp.com/callback",
        );

        assert_eq!(config.idp_type, IdpType::AzureAd);
        assert!(config.issuer_url.contains("login.microsoftonline.com"));
    }

    #[test]
    fn test_google_workspace_config() {
        let config =
            IdpConfig::google_workspace("client-123", "secret-456", "https://myapp.com/callback");

        assert_eq!(config.idp_type, IdpType::GoogleWorkspace);
        assert_eq!(config.issuer_url, "https://accounts.google.com");
    }

    #[test]
    fn test_authorization_url() {
        let config = IdpConfig::okta(
            "example.okta.com",
            "client-123",
            "secret-456",
            "https://myapp.com/callback",
        );

        let url = config.authorization_url("state-abc", "nonce-xyz");

        assert!(url.contains("client_id=client-123"));
        assert!(url.contains("state=state-abc"));
        assert!(url.contains("nonce=nonce-xyz"));
        assert!(url.contains("openid"));
    }
}
