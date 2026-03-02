//! # 核心 Trait 定义
//!
//! 定义可扩展的接口，包括通道、工具执行器、插件等。

use crate::types::{
    ChannelType, MessageContent, SessionId, ToolId, DeviceId, ExecutionContext,
    PermissionLevel,
};
use crate::error::{OpenClawError, Result, ChannelError, PluginError};
use async_trait::async_trait;
use serde_json::Value;

// ============== 通道 Trait ==============

/// 消息通道接口
///
/// 每个通道（Telegram、Slack 等）需要实现此接口。
#[async_trait]
pub trait Channel: Send + Sync + std::fmt::Debug {
    /// 获取通道类型
    fn channel_type(&self) -> ChannelType;

    /// 发送消息
    async fn send_message(
        &self,
        user_id: &str,
        message: &MessageContent,
    ) -> Result<(), ChannelError>;

    /// 验证 Webhook 请求
    async fn verify_webhook(
        &self,
        payload: &[u8],
        signature: Option<&str>,
    ) -> Result<bool, ChannelError>;

    /// 启动通道监听
    async fn start(&mut self) -> Result<()>;

    /// 停止通道监听
    async fn stop(&mut self) -> Result<()>;

    /// 获取通道是否运行中
    fn is_running(&self) -> bool;
}

// ============== 工具执行器 Trait ==============

/// 工具执行器接口
///
/// 定义工具的执行规范，包括参数验证和权限检查。
#[async_trait]
pub trait ToolExecutor: Send + Sync + std::fmt::Debug {
    /// 获取工具 ID
    fn id(&self) -> &ToolId;

    /// 获取工具名称
    fn name(&self) -> &str;

    /// 获取工具描述
    fn description(&self) -> &str;

    /// 获取参数 JSON Schema
    fn parameters_schema(&self) -> &serde_json::Value;

    /// 获取所需权限级别
    fn permission_level(&self) -> PermissionLevel;

    /// 验证参数
    fn validate_params(&self, _params: &serde_json::Value) -> Result<()> {
        // 默认实现：不做验证
        Ok(())
    }

    /// 检查权限
    fn check_permission(&self, context: &ExecutionContext) -> Result<()> {
        if self.permission_level() > PermissionLevel::Public {
            // 需要认证
            if context.user_id.is_none() && context.device_id.is_none() {
                return Err(OpenClawError::AuthFailed("Authentication required".to_string()));
            }
        }
        Ok(())
    }

    /// 执行工具
    async fn execute(
        &self,
        params: serde_json::Value,
        context: &ExecutionContext,
    ) -> Result<serde_json::Value>;
}

// ============== 插件 Trait ==============

/// 插件接口
///
/// 支持进程外插件和 WASM 插件。
#[async_trait]
pub trait Plugin: Send + Sync + std::fmt::Debug {
    /// 获取插件名称
    fn name(&self) -> &str;

    /// 获取插件版本
    fn version(&self) -> &str;

    /// 初始化插件
    async fn initialize(&mut self) -> Result<(), PluginError>;

    /// 调用插件方法
    async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, PluginError>;

    /// 关闭插件
    async fn shutdown(&mut self) -> Result<(), PluginError>;

    /// 检查插件是否健康
    async fn health_check(&self) -> Result<(), PluginError>;
}

// ============== 会话存储 Trait ==============

/// 会话持久化接口
#[async_trait]
pub trait SessionStore: Send + Sync + std::fmt::Debug {
    /// 创建会话
    async fn create_session(&self, session: &crate::types::Session) -> Result<()>;

    /// 获取会话
    async fn get_session(&self, id: &SessionId) -> Result<Option<crate::types::Session>>;

    /// 通过通道和用户 ID 获取会话
    async fn get_session_by_channel(
        &self,
        channel: ChannelType,
        channel_user_id: &str,
    ) -> Result<Option<crate::types::Session>>;

    /// 更新会话
    async fn update_session(&self, session: &crate::types::Session) -> Result<()>;

    /// 删除会话
    async fn delete_session(&self, id: &SessionId) -> Result<()>;

    /// 列出所有会话
    async fn list_sessions(&self, limit: usize, offset: usize) -> Result<Vec<crate::types::Session>>;
}

// ============== 对话历史 Trait ==============

/// 对话历史存储接口
#[async_trait]
pub trait ConversationStore: Send + Sync + std::fmt::Debug {
    /// 添加消息
    async fn add_message(&self, message: &crate::types::Message) -> Result<()>;

    /// 获取会话历史
    async fn get_history(
        &self,
        session_id: &SessionId,
        limit: usize,
        before_sequence: Option<u64>,
    ) -> Result<Vec<crate::types::Message>>;

    /// 获取最后一条消息的序号
    async fn get_last_sequence(&self, session_id: &SessionId) -> Result<Option<u64>>;

    /// 清除历史
    async fn clear_history(&self, session_id: &SessionId) -> Result<()>;
}

// ============== 工具注册表 Trait ==============

/// 工具注册表接口
#[async_trait]
pub trait ToolRegistry: Send + Sync + std::fmt::Debug {
    /// 注册工具
    async fn register_tool(&mut self, tool: Box<dyn ToolExecutor>) -> Result<()>;

    /// 注销工具
    async fn unregister_tool(&mut self, id: &ToolId) -> Result<()>;

    /// 获取工具
    async fn get_tool(&self, id: &ToolId) -> Result<Option<Box<dyn ToolExecutor>>>;

    /// 列出所有工具
    async fn list_tools(&self) -> Result<Vec<Box<dyn ToolExecutor>>>;

    /// 调用工具
    async fn call_tool(
        &self,
        id: &ToolId,
        params: serde_json::Value,
        context: &ExecutionContext,
    ) -> Result<serde_json::Value>;
}

// ============== 设备管理 Trait ==============

/// 设备管理接口
#[async_trait]
pub trait DeviceManager: Send + Sync + std::fmt::Debug {
    /// 注册设备
    async fn register_device(&self, device: &crate::types::DeviceInfo) -> Result<()>;

    /// 获取设备
    async fn get_device(&self, id: &DeviceId) -> Result<Option<crate::types::DeviceInfo>>;

    /// 更新设备信息
    async fn update_device(&self, device: &crate::types::DeviceInfo) -> Result<()>;

    /// 删除设备
    async fn delete_device(&self, id: &DeviceId) -> Result<()>;

    /// 配对设备
    async fn pair_device(&self, id: &DeviceId) -> Result<()>;

    /// 取消配对
    async fn unpair_device(&self, id: &DeviceId) -> Result<()>;

    /// 更新最后活跃时间
    async fn update_last_seen(&self, id: &DeviceId, timestamp: u64) -> Result<()>;

    /// 列出所有设备
    async fn list_devices(&self) -> Result<Vec<crate::types::DeviceInfo>>;
}

// ============== 调度器 Trait ==============

/// 任务调度器接口
#[async_trait]
pub trait Scheduler: Send + Sync + std::fmt::Debug {
    /// 添加任务
    async fn schedule_job(&mut self, job: &crate::types::ScheduledJob) -> Result<()>;

    /// 取消任务
    async fn cancel_job(&mut self, id: &crate::types::JobId) -> Result<()>;

    /// 获取任务
    async fn get_job(&self, id: &crate::types::JobId) -> Result<Option<crate::types::ScheduledJob>>;

    /// 列出任务
    async fn list_jobs(&self) -> Result<Vec<crate::types::ScheduledJob>>;

    /// 启动调度器
    async fn start(&mut self) -> Result<()>;

    /// 停止调度器
    async fn stop(&mut self) -> Result<()>;

    /// 检查运行状态
    fn is_running(&self) -> bool;
}

// ============== 认证服务 Trait ==============

/// 认证服务接口
#[async_trait]
pub trait AuthService: Send + Sync + std::fmt::Debug {
    /// 生成访问令牌
    async fn create_token(
        &self,
        device_id: Option<DeviceId>,
        user_id: Option<String>,
        scopes: Vec<String>,
        expires_in: u64,
    ) -> Result<String>;

    /// 验证令牌
    async fn verify_token(&self, token: &str) -> Result<crate::types::AccessToken>;

    /// 撤销令牌
    async fn revoke_token(&self, token: &str) -> Result<()>;

    /// 刷新令牌
    async fn refresh_token(&self, token: &str) -> Result<String>;
}

// ============== 幂等性服务 Trait ==============

/// 幂等性保证接口
#[async_trait]
pub trait IdempotencyService: Send + Sync + std::fmt::Debug {
    /// 检查并记录幂等键
    async fn check_or_record(
        &self,
        key: &str,
        ttl_seconds: u64,
    ) -> Result<Option<serde_json::Value>>;

    /// 存储响应
    async fn store_response(&self, key: &str, response: &serde_json::Value, ttl_seconds: u64) -> Result<()>;

    /// 清理过期记录
    async fn cleanup_expired(&self) -> Result<usize>;
}

// ============== 事件总线 Trait ==============

/// 事件发布订阅接口
#[async_trait]
pub trait EventBus: Send + Sync + std::fmt::Debug {
    /// 发布事件
    async fn publish(&self, event: &crate::protocol::Event) -> Result<()>;

    /// 订阅事件
    async fn subscribe(&mut self, pattern: &str) -> Result<()>;

    /// 取消订阅
    async fn unsubscribe(&mut self, pattern: &str) -> Result<()>;
}
