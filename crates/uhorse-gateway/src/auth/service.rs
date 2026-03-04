//! # Authentication Service
//!
//! 用户认证服务。

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use super::jwt::{Claims, JwtService};

/// 用户信息
#[derive(Debug, Clone)]
pub struct User {
    /// 用户 ID
    pub id: String,
    /// 用户名
    pub username: String,
    /// 密码哈希（简化实现，生产环境应使用 bcrypt/argon2）
    pub password_hash: String,
    /// 角色
    pub role: String,
}

/// 认证结果
#[derive(Debug)]
pub struct AuthResult {
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌
    pub refresh_token: String,
    /// 过期时间（秒）
    pub expires_in: u64,
}

/// 认证服务
#[derive(Debug)]
pub struct AuthService {
    jwt_service: JwtService,
    /// 用户存储（简化实现，生产环境应使用数据库）
    users: Arc<RwLock<HashMap<String, User>>>,
    /// 刷新令牌黑名单（用于登出）
    token_blacklist: Arc<RwLock<Vec<String>>>,
}

impl AuthService {
    /// 创建新的认证服务
    pub fn new(jwt_service: JwtService) -> Self {
        let users = Arc::new(RwLock::new(HashMap::new()));
        let token_blacklist = Arc::new(RwLock::new(Vec::new()));

        info!("AuthService initialized");

        Self {
            jwt_service,
            users,
            token_blacklist,
        }
    }

    /// 添加用户（异步版本）
    pub async fn add_user(&self, user: User) {
        let mut users = self.users.write().await;
        users.insert(user.username.clone(), user);
    }

    /// 登录验证
    pub async fn login(&self, username: &str, password: &str) -> Option<AuthResult> {
        // 检查用户是否存在且密码正确
        let users = self.users.read().await;

        // 如果用户存储为空，创建默认管理员
        if users.is_empty() {
            drop(users);
            let mut users_mut = self.users.write().await;
            users_mut.insert(
                "admin".to_string(),
                User {
                    id: "user-admin".to_string(),
                    username: "admin".to_string(),
                    password_hash: "admin123".to_string(), // 生产环境应使用哈希
                    role: "admin".to_string(),
                },
            );
            drop(users_mut);
        }

        let users = self.users.read().await;
        let user = users.get(username)?;

        // 简化密码验证（生产环境应使用安全的哈希比较）
        if user.password_hash != password {
            return None;
        }

        let user_id = &user.id;
        let role = &user.role;

        // 生成令牌
        let access_token = self
            .jwt_service
            .generate_access_token(user_id, username, role)
            .ok()?;
        let refresh_token = self
            .jwt_service
            .generate_refresh_token(user_id, username, role)
            .ok()?;

        Some(AuthResult {
            access_token,
            refresh_token,
            expires_in: self.jwt_service.access_token_expiry(),
        })
    }

    /// 刷新令牌
    pub async fn refresh_token(&self, refresh_token: &str) -> Option<AuthResult> {
        // 检查黑名单
        let blacklist = self.token_blacklist.read().await;
        if blacklist.contains(&refresh_token.to_string()) {
            return None;
        }
        drop(blacklist);

        // 验证刷新令牌
        let claims = self.jwt_service.verify_token(refresh_token).ok()?;

        // 确保是刷新令牌
        if claims.token_type != "refresh" {
            return None;
        }

        // 生成新令牌
        let access_token = self
            .jwt_service
            .generate_access_token(&claims.sub, &claims.username, &claims.role)
            .ok()?;
        let new_refresh_token = self
            .jwt_service
            .generate_refresh_token(&claims.sub, &claims.username, &claims.role)
            .ok()?;

        Some(AuthResult {
            access_token,
            refresh_token: new_refresh_token,
            expires_in: self.jwt_service.access_token_expiry(),
        })
    }

    /// 登出（将刷新令牌加入黑名单）
    pub async fn logout(&self, refresh_token: &str) {
        let mut blacklist = self.token_blacklist.write().await;
        blacklist.push(refresh_token.to_string());

        // 清理过期的黑名单条目（简化实现）
        if blacklist.len() > 10000 {
            // 保留最新的 5000 条
            blacklist.drain(0..5000);
        }
    }

    /// 验证访问令牌
    pub fn verify_access_token(&self, token: &str) -> Option<Claims> {
        let claims = self.jwt_service.verify_token(token).ok()?;

        if claims.token_type != "access" {
            return None;
        }

        Some(claims)
    }
}

impl Default for AuthService {
    fn default() -> Self {
        Self::new(JwtService::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_login_success() {
        let service = AuthService::default();

        // 添加测试用户
        service
            .add_user(User {
                id: "user-1".to_string(),
                username: "testuser".to_string(),
                password_hash: "password123".to_string(),
                role: "user".to_string(),
            })
            .await;

        // 测试登录
        let result = service.login("testuser", "password123").await;
        assert!(result.is_some());

        let auth_result = result.unwrap();
        assert!(!auth_result.access_token.is_empty());
        assert!(!auth_result.refresh_token.is_empty());
        assert_eq!(auth_result.expires_in, 86400); // 默认 24 小时
    }

    #[tokio::test]
    async fn test_login_wrong_password() {
        let service = AuthService::default();

        service
            .add_user(User {
                id: "user-1".to_string(),
                username: "testuser".to_string(),
                password_hash: "correct_password".to_string(),
                role: "user".to_string(),
            })
            .await;

        let result = service.login("testuser", "wrong_password").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_login_nonexistent_user() {
        let service = AuthService::default();

        let result = service.login("nonexistent", "password").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_refresh_token() {
        let service = AuthService::default();

        service
            .add_user(User {
                id: "user-1".to_string(),
                username: "testuser".to_string(),
                password_hash: "password123".to_string(),
                role: "user".to_string(),
            })
            .await;

        // 先登录获取 refresh token
        let login_result = service.login("testuser", "password123").await.unwrap();

        // 等待 1 秒确保时间戳不同（JWT 使用秒级时间戳）
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // 使用 refresh token 刷新
        let refresh_result = service.refresh_token(&login_result.refresh_token).await;
        assert!(refresh_result.is_some());

        let new_auth = refresh_result.unwrap();

        // 验证新 token 与旧 token 不同（由于时间戳不同）
        assert_ne!(new_auth.access_token, login_result.access_token);
        assert_ne!(new_auth.refresh_token, login_result.refresh_token);

        // 验证新 access token 可以正确解析
        let claims = service.verify_access_token(&new_auth.access_token);
        assert!(claims.is_some());
        let claims = claims.unwrap();
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.token_type, "access");
    }

    #[tokio::test]
    async fn test_refresh_with_access_token_fails() {
        let service = AuthService::default();

        service
            .add_user(User {
                id: "user-1".to_string(),
                username: "testuser".to_string(),
                password_hash: "password123".to_string(),
                role: "user".to_string(),
            })
            .await;

        let login_result = service.login("testuser", "password123").await.unwrap();

        // 尝试使用 access token 刷新应该失败
        let result = service.refresh_token(&login_result.access_token).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_logout_invalidates_refresh_token() {
        let service = AuthService::default();

        service
            .add_user(User {
                id: "user-1".to_string(),
                username: "testuser".to_string(),
                password_hash: "password123".to_string(),
                role: "user".to_string(),
            })
            .await;

        let login_result = service.login("testuser", "password123").await.unwrap();

        // 登出
        service.logout(&login_result.refresh_token).await;

        // 使用已登出的 refresh token 刷新应该失败
        let result = service.refresh_token(&login_result.refresh_token).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_verify_access_token() {
        let service = AuthService::default();

        service
            .add_user(User {
                id: "user-1".to_string(),
                username: "testuser".to_string(),
                password_hash: "password123".to_string(),
                role: "admin".to_string(),
            })
            .await;

        let login_result = service.login("testuser", "password123").await.unwrap();

        // 验证 access token
        let claims = service.verify_access_token(&login_result.access_token);
        assert!(claims.is_some());

        let claims = claims.unwrap();
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.role, "admin");
    }

    #[tokio::test]
    async fn test_verify_refresh_token_as_access_fails() {
        let service = AuthService::default();

        service
            .add_user(User {
                id: "user-1".to_string(),
                username: "testuser".to_string(),
                password_hash: "password123".to_string(),
                role: "user".to_string(),
            })
            .await;

        let login_result = service.login("testuser", "password123").await.unwrap();

        // 使用 refresh token 作为 access token 验证应该失败
        let result = service.verify_access_token(&login_result.refresh_token);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_default_admin_user() {
        let service = AuthService::default();

        // 首次登录会创建默认管理员
        let result = service.login("admin", "admin123").await;
        assert!(result.is_some());

        let claims = service
            .verify_access_token(&result.unwrap().access_token)
            .unwrap();
        assert_eq!(claims.role, "admin");
    }

    #[tokio::test]
    async fn test_multiple_users() {
        let service = AuthService::default();

        // 添加多个用户
        service
            .add_user(User {
                id: "user-1".to_string(),
                username: "alice".to_string(),
                password_hash: "pass1".to_string(),
                role: "admin".to_string(),
            })
            .await;

        service
            .add_user(User {
                id: "user-2".to_string(),
                username: "bob".to_string(),
                password_hash: "pass2".to_string(),
                role: "user".to_string(),
            })
            .await;

        // 测试两个用户都能登录
        let result1 = service.login("alice", "pass1").await;
        let result2 = service.login("bob", "pass2").await;

        assert!(result1.is_some());
        assert!(result2.is_some());

        // 验证不同的 token
        let claims1 = service
            .verify_access_token(&result1.unwrap().access_token)
            .unwrap();
        let claims2 = service
            .verify_access_token(&result2.unwrap().access_token)
            .unwrap();

        assert_ne!(claims1.sub, claims2.sub);
        assert_eq!(claims1.role, "admin");
        assert_eq!(claims2.role, "user");
    }
}
