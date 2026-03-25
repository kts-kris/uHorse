//! # Gateway - 控制平面
//!
//! 单一真相来源，负责会话管理、消息路由和多通道统一接口。
//!
//! ## 职责
//!
//! - **会话管理**：创建、获取、更新、删除会话
//! - **消息路由**：将消息路由到正确的 Agent
//! - **多通道统一**：为不同通道（Telegram、Discord等）提供统一接口
//! - **事件驱动**：通过事件总线处理异步事件

use crate::agent::{Agent, AgentResponse};
use crate::error::{AgentError, AgentResult};
use crate::memory::MemoryStore;
use crate::router::Router;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uhorse_core::{Session, SessionId};
use uhorse_llm::LLMClient;

/// Gateway 配置
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// 工作空间目录
    pub workspace_dir: PathBuf,
    /// 最大会话数
    pub max_sessions: usize,
    /// 会话超时时间（秒）
    pub session_timeout: u64,
    /// 是否启用内存持久化
    pub enable_memory_persistence: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            workspace_dir: PathBuf::from("~/.uhorse/workspace"),
            max_sessions: 1000,
            session_timeout: 3600,
            enable_memory_persistence: true,
        }
    }
}

/// Gateway 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GatewayEvent {
    /// 消息接收
    MessageReceived {
        /// 关联会话 ID。
        session_id: String,
        /// 接收到的消息内容。
        message: String,
    },
    /// 消息发送
    MessageSent {
        /// 关联会话 ID。
        session_id: String,
        /// 发送的消息内容。
        message: String,
    },
    /// Agent 切换
    AgentSwitched {
        /// 关联会话 ID。
        session_id: String,
        /// 原 Agent 名称。
        from_agent: String,
        /// 新 Agent 名称。
        to_agent: String,
    },
    /// 技能调用
    SkillInvoked {
        /// 关联会话 ID。
        session_id: String,
        /// 被调用的技能名称。
        skill: String,
    },
    /// 错误
    Error {
        /// 关联会话 ID。
        session_id: String,
        /// 错误详情。
        error: String,
    },
}

/// Gateway - 控制平面
///
/// 单一真相来源，负责会话管理、消息路由和多通道统一接口。
///
/// ## 示例
///
/// ```ignore
/// use uhorse_agent::Gateway;
///
/// let gateway = Gateway::new(config, llm_client, memory).await?;
///
/// // 处理消息
/// let response = gateway.handle_message(session_id, user_message).await?;
/// ```
pub struct Gateway<C>
where
    C: LLMClient + Send + Sync,
{
    /// 配置
    config: GatewayConfig,
    /// LLM 客户端
    llm_client: Arc<C>,
    /// 内存存储
    memory: Arc<dyn MemoryStore>,
    /// 路由器
    router: Arc<Router>,
    /// Agent 注册表
    agents: Arc<RwLock<HashMap<String, Agent>>>,
    /// 会话存储
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    /// 事件发送器
    event_sender: tokio::sync::mpsc::UnboundedSender<GatewayEvent>,
}

impl<C> Gateway<C>
where
    C: LLMClient + Send + Sync + Sized + 'static,
{
    /// 创建新的 Gateway
    pub async fn new(
        config: GatewayConfig,
        llm_client: Arc<C>,
        memory: Arc<dyn MemoryStore>,
    ) -> AgentResult<Self> {
        let (event_sender, mut event_receiver) = tokio::sync::mpsc::unbounded_channel();

        // 创建路由器
        let router = Router::new();

        let gateway = Self {
            config,
            llm_client,
            memory,
            router: Arc::new(router),
            agents: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
        };

        // 启动事件处理任务
        tokio::spawn(async move {
            while let Some(event) = event_receiver.recv().await {
                // 处理事件（日志、监控等）
                tracing::debug!("Gateway event: {:?}", event);
            }
        });

        Ok(gateway)
    }

    /// 注册 Agent
    pub async fn register_agent(&self, agent: Agent) -> AgentResult<()> {
        let name = agent.name().to_string();
        let mut agents = self.agents.write().await;
        agents.insert(name.clone(), agent);
        tracing::info!("Agent registered: {}", name);
        Ok(())
    }

    /// 获取或创建会话
    pub async fn get_or_create_session(
        &self,
        channel_user_id: &str,
    ) -> AgentResult<(SessionId, bool)> {
        // 首先尝试查找现有会话
        {
            let sessions = self.sessions.read().await;
            for (id, session) in sessions.iter() {
                if session.channel_user_id == channel_user_id {
                    return Ok((id.clone(), false));
                }
            }
        }

        // 创建新会话
        let session_id = SessionId::new();
        let session = Session::new(
            uhorse_core::ChannelType::Telegram, // 默认，可扩展
            channel_user_id.to_string(),
        );

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session);

        tracing::info!(
            "New session created: {} for user {}",
            session_id,
            channel_user_id
        );
        Ok((session_id, true))
    }

    /// 处理消息
    ///
    /// 这是 Gateway 的核心方法，负责：
    /// 1. 接收消息
    /// 2. 路由到正确的 Agent
    /// 3. 返回响应
    pub async fn handle_message(
        &self,
        session_id: &SessionId,
        message: &str,
    ) -> AgentResult<AgentResponse> {
        // 发送消息接收事件
        let _ = self.event_sender.send(GatewayEvent::MessageReceived {
            session_id: session_id.to_string(),
            message: message.to_string(),
        });

        // 获取会话
        let session = {
            let sessions = self.sessions.read().await;
            sessions
                .get(session_id)
                .cloned()
                .ok_or_else(|| AgentError::SessionNotFound(session_id.to_string()))?
        };

        // 获取当前 Agent
        let agents = self.agents.read().await;
        let current_agent_name = session
            .metadata
            .get("current_agent")
            .cloned()
            .unwrap_or_else(|| {
                // 如果没有设置，使用第一个可用的 agent
                agents
                    .keys()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| "default".to_string())
            });

        let agent = agents
            .get(&current_agent_name)
            .cloned()
            .ok_or_else(|| AgentError::Agent(format!("Agent not found: {}", current_agent_name)))?;

        // 调用 Agent 处理消息
        let response = agent
            .process(
                session_id.clone(),
                message,
                self.llm_client.clone(),
                self.memory.clone(),
            )
            .await?;

        // 发送消息发送事件
        let _ = self.event_sender.send(GatewayEvent::MessageSent {
            session_id: session_id.to_string(),
            message: response.content.clone(),
        });

        Ok(response)
    }

    /// 获取配置。
    pub fn config(&self) -> &GatewayConfig {
        &self.config
    }

    /// 获取路由器。
    pub fn router(&self) -> &Router {
        &self.router
    }

    /// 获取所有会话
    pub async fn list_sessions(&self) -> Vec<Session> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }

    /// 获取事件接收器
    pub fn event_receiver(&self) -> tokio::sync::mpsc::UnboundedReceiver<GatewayEvent> {
        // 注意：这会创建一个新的接收器，实际使用可能需要更复杂的设计
        todo!("Return event receiver")
    }
}
