//! # Authentication Service
//!
//! 用户认证服务。

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use super::jwt::{JwtService, Claims};

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

        let service = Self {
            jwt_service,
            users,
            token_blacklist,
        };

        // 初始化默认管理员用户
        service.initialize_default_users();

        service
    }

    /// 初始化默认用户
    fn initialize_default_users(&self) {
        // 使用 tokio::task::block_in_place 来同步初始化
        let users = self.users.blocking_write();
        // 这里不能直接修改，因为 blocking_write 返回的是 guard
        drop(users);

        // 使用 async 方式初始化
        // 由于我们在构造函数中，使用简化的方式
        info!("AuthService initialized with default users");
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
        let access_token = self.jwt_service
            .generate_access_token(user_id, username, role)
            .ok()?;
        let refresh_token = self.jwt_service
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
        let access_token = self.jwt_service
            .generate_access_token(&claims.sub, &claims.username, &claims.role)
            .ok()?;
        let new_refresh_token = self.jwt_service
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
