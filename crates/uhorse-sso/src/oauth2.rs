//! OAuth2 Authorization Server
//!
//! 实现 OAuth2 授权码和客户端凭证流程

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// OAuth2 配置
#[derive(Debug, Clone)]
pub struct OAuth2Config {
    /// 授权端点路径
    pub authorize_endpoint: String,
    /// 令牌端点路径
    pub token_endpoint: String,
    /// 授权码有效期 (秒)
    pub auth_code_ttl: u64,
    /// 访问令牌有效期 (秒)
    pub access_token_ttl: u64,
    /// 刷新令牌有效期 (秒)
    pub refresh_token_ttl: u64,
    /// 是否启用刷新令牌
    pub enable_refresh_token: bool,
    /// 支持的授权类型
    pub grant_types: Vec<String>,
    /// 支持的响应类型
    pub response_types: Vec<String>,
}

impl Default for OAuth2Config {
    fn default() -> Self {
        Self {
            authorize_endpoint: "/oauth2/authorize".to_string(),
            token_endpoint: "/oauth2/token".to_string(),
            auth_code_ttl: 600,           // 10 分钟
            access_token_ttl: 3600,       // 1 小时
            refresh_token_ttl: 86400 * 7, // 7 天
            enable_refresh_token: true,
            grant_types: vec![
                "authorization_code".to_string(),
                "client_credentials".to_string(),
                "refresh_token".to_string(),
            ],
            response_types: vec!["code".to_string()],
        }
    }
}

/// OAuth2 客户端
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2Client {
    /// 客户端 ID
    pub client_id: String,
    /// 客户端密钥
    pub client_secret: String,
    /// 重定向 URI
    pub redirect_uris: Vec<String>,
    /// 客户端名称
    pub name: String,
    /// 支持的授权类型
    pub grant_types: Vec<String>,
    /// 支持的作用域
    pub scope: Vec<String>,
    /// 是否启用
    pub enabled: bool,
}

impl OAuth2Client {
    /// 创建新的客户端
    pub fn new(name: impl Into<String>, redirect_uris: Vec<String>) -> Self {
        Self {
            client_id: uuid::Uuid::new_v4().to_string(),
            client_secret: generate_client_secret(),
            redirect_uris,
            name: name.into(),
            grant_types: vec!["authorization_code".to_string()],
            scope: vec!["openid".to_string(), "profile".to_string()],
            enabled: true,
        }
    }

    /// 验证重定向 URI
    pub fn validate_redirect_uri(&self, uri: &str) -> bool {
        self.redirect_uris.iter().any(|u| u == uri)
    }

    /// 验证密钥
    pub fn validate_secret(&self, secret: &str) -> bool {
        self.client_secret == secret
    }
}

/// 授权码
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationCode {
    /// 授权码
    pub code: String,
    /// 客户端 ID
    pub client_id: String,
    /// 重定向 URI
    pub redirect_uri: String,
    /// 用户 ID
    pub user_id: String,
    /// 作用域
    pub scope: Vec<String>,
    /// 状态
    pub state: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 过期时间
    pub expires_at: DateTime<Utc>,
    /// 是否已使用
    pub used: bool,
}

impl AuthorizationCode {
    /// 创建新的授权码
    pub fn new(
        client_id: impl Into<String>,
        redirect_uri: impl Into<String>,
        user_id: impl Into<String>,
        scope: Vec<String>,
        ttl_secs: u64,
    ) -> Self {
        let now = Utc::now();
        Self {
            code: uuid::Uuid::new_v4().to_string(),
            client_id: client_id.into(),
            redirect_uri: redirect_uri.into(),
            user_id: user_id.into(),
            scope,
            state: None,
            created_at: now,
            expires_at: now + chrono::Duration::seconds(ttl_secs as i64),
            used: false,
        }
    }

    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// 标记为已使用
    pub fn mark_used(&mut self) {
        self.used = true;
    }
}

/// 令牌响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    /// 访问令牌
    pub access_token: String,
    /// 令牌类型
    pub token_type: String,
    /// 过期时间 (秒)
    pub expires_in: u64,
    /// 刷新令牌
    pub refresh_token: Option<String>,
    /// 作用域
    pub scope: Option<String>,
    /// ID 令牌 (OIDC)
    pub id_token: Option<String>,
}

/// 令牌信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌
    pub refresh_token: Option<String>,
    /// 客户端 ID
    pub client_id: String,
    /// 用户 ID
    pub user_id: String,
    /// 作用域
    pub scope: Vec<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 过期时间
    pub expires_at: DateTime<Utc>,
}

impl TokenInfo {
    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

/// OAuth2 授权服务器
pub struct OAuth2Server {
    /// 配置
    config: OAuth2Config,
    /// 已注册的客户端
    clients: Arc<RwLock<HashMap<String, OAuth2Client>>>,
    /// 授权码存储
    auth_codes: Arc<RwLock<HashMap<String, AuthorizationCode>>>,
    /// 令牌存储
    tokens: Arc<RwLock<HashMap<String, TokenInfo>>>,
}

impl OAuth2Server {
    /// 创建新的授权服务器
    pub fn new(config: OAuth2Config) -> Self {
        Self {
            config,
            clients: Arc::new(RwLock::new(HashMap::new())),
            auth_codes: Arc::new(RwLock::new(HashMap::new())),
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册客户端
    pub async fn register_client(&self, client: OAuth2Client) {
        let client_id = client.client_id.clone();
        let mut clients = self.clients.write().await;
        clients.insert(client_id, client);
        info!("Registered OAuth2 client");
    }

    /// 获取客户端
    pub async fn get_client(&self, client_id: &str) -> Option<OAuth2Client> {
        let clients = self.clients.read().await;
        clients.get(client_id).cloned()
    }

    /// 创建授权码
    pub async fn create_auth_code(
        &self,
        client_id: &str,
        redirect_uri: &str,
        user_id: &str,
        scope: Vec<String>,
    ) -> crate::Result<AuthorizationCode> {
        let code = AuthorizationCode::new(
            client_id,
            redirect_uri,
            user_id,
            scope,
            self.config.auth_code_ttl,
        );

        let code_str = code.code.clone();
        let mut codes = self.auth_codes.write().await;
        codes.insert(code_str, code.clone());

        Ok(code)
    }

    /// 使用授权码交换令牌
    pub async fn exchange_auth_code(
        &self,
        code: &str,
        client_id: &str,
        redirect_uri: &str,
    ) -> crate::Result<TokenResponse> {
        let mut codes = self.auth_codes.write().await;

        let auth_code = codes.get_mut(code).ok_or_else(|| {
            crate::SsoError::InvalidState("Invalid authorization code".to_string())
        })?;

        // 验证授权码
        if auth_code.client_id != client_id {
            return Err(crate::SsoError::AuthenticationFailed(
                "Client ID mismatch".to_string(),
            ));
        }

        if auth_code.redirect_uri != redirect_uri {
            return Err(crate::SsoError::AuthenticationFailed(
                "Redirect URI mismatch".to_string(),
            ));
        }

        if auth_code.used {
            return Err(crate::SsoError::InvalidState(
                "Authorization code already used".to_string(),
            ));
        }

        if auth_code.is_expired() {
            return Err(crate::SsoError::InvalidState(
                "Authorization code expired".to_string(),
            ));
        }

        // 标记为已使用
        auth_code.mark_used();

        // 生成令牌
        self.generate_token_response(client_id, &auth_code.user_id, auth_code.scope.clone())
            .await
    }

    /// 刷新令牌
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
        client_id: &str,
    ) -> crate::Result<TokenResponse> {
        let tokens = self.tokens.read().await;

        let token_info = tokens
            .get(refresh_token)
            .ok_or_else(|| crate::SsoError::InvalidState("Invalid refresh token".to_string()))?;

        if token_info.client_id != client_id {
            return Err(crate::SsoError::AuthenticationFailed(
                "Client ID mismatch".to_string(),
            ));
        }

        if token_info.is_expired() {
            return Err(crate::SsoError::InvalidState(
                "Refresh token expired".to_string(),
            ));
        }

        // 生成新令牌
        let user_id = token_info.user_id.clone();
        let scope = token_info.scope.clone();

        drop(tokens); // 释放锁

        self.generate_token_response(client_id, &user_id, scope)
            .await
    }

    /// 生成令牌响应
    async fn generate_token_response(
        &self,
        client_id: &str,
        user_id: &str,
        scope: Vec<String>,
    ) -> crate::Result<TokenResponse> {
        let access_token = generate_access_token();
        let refresh_token = if self.config.enable_refresh_token {
            Some(generate_refresh_token())
        } else {
            None
        };

        let now = Utc::now();
        let token_info = TokenInfo {
            access_token: access_token.clone(),
            refresh_token: refresh_token.clone(),
            client_id: client_id.to_string(),
            user_id: user_id.to_string(),
            scope: scope.clone(),
            created_at: now,
            expires_at: now + chrono::Duration::seconds(self.config.access_token_ttl as i64),
        };

        let mut tokens = self.tokens.write().await;
        tokens.insert(access_token.clone(), token_info);

        if let Some(ref refresh) = refresh_token {
            let refresh_info = TokenInfo {
                access_token: access_token.clone(),
                refresh_token: Some(refresh.clone()),
                client_id: client_id.to_string(),
                user_id: user_id.to_string(),
                scope: scope.clone(),
                created_at: now,
                expires_at: now + chrono::Duration::seconds(self.config.refresh_token_ttl as i64),
            };
            tokens.insert(refresh.clone(), refresh_info);
        }

        Ok(TokenResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.access_token_ttl,
            refresh_token,
            scope: Some(scope.join(" ")),
            id_token: None,
        })
    }

    /// 验证访问令牌
    pub async fn validate_token(&self, access_token: &str) -> crate::Result<TokenInfo> {
        let tokens = self.tokens.read().await;
        let token_info = tokens
            .get(access_token)
            .ok_or_else(|| crate::SsoError::TokenError("Invalid access token".to_string()))?;

        if token_info.is_expired() {
            return Err(crate::SsoError::TokenError(
                "Access token expired".to_string(),
            ));
        }

        Ok(token_info.clone())
    }

    /// 撤销令牌
    pub async fn revoke_token(&self, token: &str) -> crate::Result<bool> {
        let mut tokens = self.tokens.write().await;
        Ok(tokens.remove(token).is_some())
    }
}

impl Default for OAuth2Server {
    fn default() -> Self {
        Self::new(OAuth2Config::default())
    }
}

/// 生成客户端密钥
fn generate_client_secret() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &bytes)
}

/// 生成访问令牌
fn generate_access_token() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}

/// 生成刷新令牌
fn generate_refresh_token() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth2_client() {
        let client = OAuth2Client::new("Test App", vec!["http://localhost/callback".to_string()]);

        assert!(client.validate_redirect_uri("http://localhost/callback"));
        assert!(!client.validate_redirect_uri("http://evil.com/callback"));
    }

    #[test]
    fn test_authorization_code() {
        let code = AuthorizationCode::new(
            "client-1",
            "http://localhost/callback",
            "user-1",
            vec!["openid".to_string()],
            600,
        );

        assert!(!code.is_expired());
        assert!(!code.used);
    }

    #[tokio::test]
    async fn test_oauth2_server() {
        let server = OAuth2Server::default();

        // 注册客户端
        let client = OAuth2Client::new("Test App", vec!["http://localhost/callback".to_string()]);
        let client_id = client.client_id.clone();
        server.register_client(client).await;

        // 创建授权码
        let code = server
            .create_auth_code(
                &client_id,
                "http://localhost/callback",
                "user-1",
                vec!["openid".to_string()],
            )
            .await
            .unwrap();

        // 交换令牌
        let token = server
            .exchange_auth_code(&code.code, &client_id, "http://localhost/callback")
            .await
            .unwrap();

        assert!(!token.access_token.is_empty());
        assert_eq!(token.token_type, "Bearer");
    }
}
