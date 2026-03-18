//! # uHorse SSO Module
//!
//! SSO/OAuth2/OIDC/SAML 集成模块
//!
//! ## Features
//!
//! - OAuth2 授权服务器
//! - OIDC 身份发现
//! - SAML 2.0 企业 SSO
//! - 多 IdP 集成 (Okta/Auth0/Azure AD)

pub mod idp;
pub mod oauth2;
pub mod oidc;
pub mod saml;

pub use idp::{IdpClient, IdpConfig, IdpType};
pub use oauth2::{AuthorizationCode, OAuth2Config, OAuth2Server, TokenResponse};
pub use oidc::{OidcClient, OidcConfig, UserInfo};
pub use saml::{SamlClient, SamlConfig, SamlResponse};

use thiserror::Error;

/// SSO 错误类型
#[derive(Error, Debug)]
pub enum SsoError {
    #[error("OAuth2 error: {0}")]
    OAuth2Error(String),

    #[error("OIDC error: {0}")]
    OidcError(String),

    #[error("SAML error: {0}")]
    SamlError(String),

    #[error("IdP error: {0}")]
    IdpError(String),

    #[error("Token error: {0}")]
    TokenError(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// SSO 结果类型
pub type Result<T> = std::result::Result<T, SsoError>;
