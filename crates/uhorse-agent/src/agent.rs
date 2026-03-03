//! # Agent - 智能体
//!
//! OpenClaw 风格的 Agent 实现，每个 Agent 有独立的 workspace。
//!
//! ## 核心能力
//!
//! - **独立 Workspace**：每个 Agent 有自己的 SOUL.md、MEMORY.md
//! - **LLM 调用**：与大语言模型交互
//! - **工具/技能使用**：调用注册的技能
//! - **意图识别**：理解用户意图
//! - **多 Agent 协作**：与其他 Agent 协作完成复杂任务

use crate::agent_scope::AgentScope;
use crate::error::{AgentError, AgentResult};
use crate::memory::MemoryStore;
use crate::skill::SkillRegistry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uhorse_core::SessionId;
use uhorse_llm::{ChatMessage, LLMClient};

/// Agent 配置
#[derive(Clone)]
pub struct AgentConfig {
    /// Agent ID（用于路由和状态管理）
    pub agent_id: String,
    /// Agent 名称（显示用）
    pub name: String,
    /// Agent 描述
    pub description: String,
    /// 系统提示词
    pub system_prompt: String,
    /// Workspace 目录
    pub workspace_dir: PathBuf,
    /// 使用的模型
    pub model: Option<String>,
    /// 温度参数
    pub temperature: Option<f32>,
    /// 最大 token 数
    pub max_tokens: Option<u32>,
    /// 技能注册表
    pub skills: SkillRegistry,
    /// 是否是默认 Agent
    pub is_default: bool,
}

/// Agent 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    /// 响应内容
    pub content: String,
    /// 使用的技能
    pub skills_used: Vec<String>,
    /// 是否需要移交到其他 Agent
    pub needs_handoff: Option<String>,
    /// 额外数据
    pub metadata: HashMap<String, String>,
}

/// Agent - 智能体
///
/// OpenClaw 风格的 Agent，拥有独立的 workspace 和作用域。
///
/// ## 示例
///
/// ```ignore
/// use uhorse_agent::{Agent, AgentScope};
///
/// // 创建 Agent Scope
/// let scope = AgentScope::new(agent_scope_config)?;
/// scope.init_workspace().await?;
///
/// // 创建 Agent
/// let agent = Agent::builder()
///     .agent_id("coder")
///     .name("Code Assistant")
///     .workspace_dir("~/.uhorse/workspace-coder")
///     .system_prompt("You are an expert programmer.")
///     .build()?;
/// ```
#[derive(Clone)]
pub struct Agent {
    /// 配置
    config: AgentConfig,
    /// Agent 作用域
    scope: Option<Arc<AgentScope>>,
}

impl Agent {
    /// 创建 Agent 构建器
    pub fn builder() -> AgentBuilder {
        AgentBuilder::default()
    }

    /// 获取 Agent ID
    pub fn agent_id(&self) -> &str {
        &self.config.agent_id
    }

    /// 获取 Agent 名称
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// 获取 Agent 描述
    pub fn description(&self) -> &str {
        &self.config.description
    }

    /// 获取 workspace 目录
    pub fn workspace_dir(&self) -> &PathBuf {
        &self.config.workspace_dir
    }

    /// 设置 Agent 作用域
    pub fn with_scope(mut self, scope: AgentScope) -> Self {
        self.scope = Some(Arc::new(scope));
        self
    }

    /// 获取 Agent 作用域
    pub fn scope(&self) -> Option<&AgentScope> {
        self.scope.as_ref().map(|s| s.as_ref())
    }

    /// 处理消息
    pub async fn process<C>(
        &self,
        session_id: SessionId,
        message: &str,
        llm_client: Arc<C>,
        memory: Arc<dyn MemoryStore>,
    ) -> AgentResult<AgentResponse>
    where
        C: LLMClient + Send + Sync,
    {
        // 1. 从内存获取上下文
        let memory_context = memory.get_context(&session_id).await.unwrap_or_default();

        // 2. 从 scope 获取注入的文件（如果有）
        let injected_context = if let Some(scope) = &self.scope {
            let files = scope
                .get_injected_files(&session_id, true) // TODO: 判断是否是主会话
                .await
                .unwrap_or_default();

            if !files.is_empty() {
                let mut context = String::from("--- Context from Agent Workspace ---\n");
                for (name, content) in files {
                    context.push_str(&format!("\n## {}\n{}\n", name, content));
                }
                context
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // 3. 构建消息历史
        let mut messages = vec![ChatMessage::system(self.config.system_prompt.clone())];

        // 添加 workspace 注入的上下文
        if !injected_context.is_empty() {
            messages.push(ChatMessage::system(injected_context));
        }

        // 添加记忆上下文
        if !memory_context.is_empty() {
            messages.push(ChatMessage::system(format!(
                "Relevant context:\n{}",
                memory_context
            )));
        }

        // 添加用户消息
        messages.push(ChatMessage::user(message.to_string()));

        // 4. 调用 LLM
        let response_content = llm_client
            .chat_completion(messages)
            .await
            .map_err(|e| AgentError::LLM(e.to_string()))?;

        // 5. 检测是否需要调用技能
        let skills_used = self.detect_skill_usage(&response_content).await;

        // 6. 如果需要技能，执行技能
        let final_content = if !skills_used.is_empty() {
            self.execute_skills(&skills_used, &session_id, message)
                .await?
        } else {
            response_content.clone()
        };

        // 7. 保存到今日 memory
        if let Some(scope) = &self.scope {
            let entry = format!(
                "## {}\n\n**User:** {}\n\n**Assistant:** {}\n\n",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
                message,
                final_content
            );
            let _ = scope.append_to_today_memory(&entry).await;
        }

        // 8. 保存到记忆
        memory
            .store_message(&session_id, message, &final_content)
            .await?;

        Ok(AgentResponse {
            content: final_content,
            skills_used,
            needs_handoff: None,
            metadata: HashMap::new(),
        })
    }

    /// 检测技能使用
    async fn detect_skill_usage(&self, content: &str) -> Vec<String> {
        // 简单实现：检查是否包含技能标记
        let mut skills_used = Vec::new();

        for skill_name in self.config.skills.list_names() {
            if content.contains(&format!("[{}]", skill_name)) {
                skills_used.push(skill_name);
            }
        }

        skills_used
    }

    /// 执行技能
    async fn execute_skills(
        &self,
        skill_names: &[String],
        session_id: &SessionId,
        input: &str,
    ) -> AgentResult<String> {
        let mut results = Vec::new();

        for skill_name in skill_names {
            if let Some(skill) = self.config.skills.get(skill_name) {
                let result = skill.execute(input).await?;
                results.push(format!("{}: {}", skill_name, result));
            }
        }

        Ok(results.join("\n"))
    }
}

/// Agent 构建器
#[derive(Default)]
pub struct AgentBuilder {
    agent_id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    system_prompt: Option<String>,
    workspace_dir: Option<PathBuf>,
    model: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    skills: SkillRegistry,
    is_default: bool,
}

impl AgentBuilder {
    /// 设置 Agent ID
    pub fn agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// 设置名称
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// 设置描述
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 设置系统提示词
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// 设置 workspace 目录
    pub fn workspace_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.workspace_dir = Some(dir.into());
        self
    }

    /// 设置模型
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// 设置温度
    pub fn temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// 设置最大 token 数
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// 添加技能
    pub fn add_skill(mut self, skill: crate::skill::Skill) -> Self {
        self.skills.register(skill);
        self
    }

    /// 设置为默认 Agent
    pub fn set_default(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }

    /// 构建 Agent
    pub fn build(self) -> AgentResult<Agent> {
        let agent_id = self.agent_id.unwrap_or_else(|| {
            self.name
                .as_ref()
                .map(|n| n.to_lowercase().replace(' ', "-"))
                .unwrap_or_else(|| "main".to_string())
        });

        let name = self
            .name
            .ok_or_else(|| AgentError::InvalidConfig("Agent name is required".to_string()))?;

        let description = self.description.unwrap_or_default();
        let system_prompt = self
            .system_prompt
            .unwrap_or_else(|| "You are a helpful AI assistant.".to_string());

        let workspace_dir = self.workspace_dir.unwrap_or_else(|| {
            if self.is_default {
                PathBuf::from("~/.uhorse/workspace")
            } else {
                PathBuf::from(format!("~/.uhorse/workspace-{}", agent_id))
            }
        });

        Ok(Agent {
            config: AgentConfig {
                agent_id,
                name,
                description,
                system_prompt,
                workspace_dir,
                model: self.model,
                temperature: self.temperature,
                max_tokens: self.max_tokens,
                skills: self.skills,
                is_default: self.is_default,
            },
            scope: None,
        })
    }
}
