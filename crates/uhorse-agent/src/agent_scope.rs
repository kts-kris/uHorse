//! # Agent Scope - Agent 作用域管理
//!
//! OpenClaw 风格的 Agent 作用域管理，每个 Agent 有独立的 workspace。
//!
//! ## 目录结构
//!
//! ```text
//! ~/.uhorse/
//! ├── workspace/              # 默认 Agent 的 workspace
//! │   ├── SOUL.md
//! │   ├── MEMORY.md
//! │   ├── AGENTS.md
//! │   ├── USER.md
//! │   └── memory/
//! │       ├── 2026-03-02.md
//! │       └── 2026-03-03.md
//! │
//! ├── workspace-coder/        # Coder Agent 的独立 workspace
//! │   ├── SOUL.md
//! │   ├── MEMORY.md
//! │   └── ...
//! │
//! └── agents/
//!     ├── {agent_id}/
//!     │   └── sessions/
//!     │       └── {session_key}/
//!     │           └── state.json
//!     └── main/
//!         └── sessions/
//! ```
//!
//! ## 文件注入优先级
//!
//! | 文件 | 注入时机 | 谁可修改 |
//! |------|----------|----------|
//! | AGENTS.md | 每个会话 | 仅人类 |
//! | SOUL.md | 每个会话 | Agent |
//! | MEMORY.md | 仅主会话 | Agent |
//! | memory/YYYY-MM-DD.md | 每个会话 | Agent |

use crate::error::{AgentError, AgentResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uhorse_core::SessionId;

/// Agent 作用域配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentScopeConfig {
    /// Agent ID
    pub agent_id: String,
    /// Workspace 目录
    pub workspace_dir: PathBuf,
    /// Agent 显示名称
    pub display_name: Option<String>,
    /// 是否是默认 Agent
    pub is_default: bool,
}

impl Default for AgentScopeConfig {
    fn default() -> Self {
        Self {
            agent_id: "main".to_string(),
            workspace_dir: PathBuf::from("~/.uhorse/workspace"),
            display_name: Some("Main Agent".to_string()),
            is_default: true,
        }
    }
}

/// Agent 作用域
///
/// 管理 Agent 的独立 workspace、文件注入和会话存储。
#[derive(Clone)]
pub struct AgentScope {
    /// 配置
    config: AgentScopeConfig,
    /// 工作空间目录（展开 ~ 后）
    workspace_dir: PathBuf,
    /// Agent 状态目录
    agents_dir: PathBuf,
}

impl AgentScope {
    /// 创建新的 Agent 作用域
    pub fn new(config: AgentScopeConfig) -> AgentResult<Self> {
        // 展开 ~
        let workspace_str = config.workspace_dir.to_string_lossy().to_string();
        let expanded = shellexpand::tilde(&workspace_str);
        let workspace_dir = PathBuf::from(expanded.as_ref());

        // agents 目录
        let agents_dir = workspace_dir
            .parent()
            .unwrap_or(&workspace_dir)
            .join("agents")
            .join(&config.agent_id);

        Ok(Self {
            config,
            workspace_dir,
            agents_dir,
        })
    }

    /// 获取配置
    pub fn config(&self) -> &AgentScopeConfig {
        &self.config
    }

    /// 获取 workspace 目录
    pub fn workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }

    /// 获取 agents 目录
    pub fn agents_dir(&self) -> &Path {
        &self.agents_dir
    }

    /// 初始化 workspace
    ///
    /// 创建必要的目录和初始文件。
    pub async fn init_workspace(&self) -> AgentResult<()> {
        // 创建 workspace 目录
        tokio::fs::create_dir_all(&self.workspace_dir)
            .await
            .map_err(|e| AgentError::Memory(format!("Failed to create workspace: {}", e)))?;

        // 创建 agents 目录
        tokio::fs::create_dir_all(&self.agents_dir)
            .await
            .map_err(|e| AgentError::Memory(format!("Failed to create agents dir: {}", e)))?;

        // 创建核心文件
        self.init_file("SOUL.md", "# Soul / Personality\n\nThis file defines the agent's personality and behavior patterns.\n").await?;
        self.init_file("MEMORY.md", "# Long-term Memory\n\nThis file stores long-term memories and important information.\n").await?;
        self.init_file("AGENTS.md", "# Agent Instructions\n\nThis file contains operating instructions and system rules. Only humans should modify this file.\n").await?;
        self.init_file(
            "USER.md",
            "# User Preferences\n\nThis file stores user preferences and settings.\n",
        )
        .await?;

        // 创建 memory 目录
        let memory_dir = self.workspace_dir.join("memory");
        tokio::fs::create_dir_all(&memory_dir)
            .await
            .map_err(|e| AgentError::Memory(format!("Failed to create memory dir: {}", e)))?;

        Ok(())
    }

    /// 初始化文件（如果不存在）
    async fn init_file(&self, name: &str, content: &str) -> AgentResult<()> {
        let path = self.workspace_dir.join(name);
        if !path.exists() {
            tokio::fs::write(&path, content)
                .await
                .map_err(|e| AgentError::Memory(format!("Failed to write {}: {}", name, e)))?;
        }
        Ok(())
    }

    /// 获取会话目录
    pub fn session_dir(&self, session_key: &str) -> PathBuf {
        self.agents_dir.join("sessions").join(session_key)
    }

    /// 获取今日 memory 文件
    pub fn today_memory_file(&self) -> PathBuf {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        self.workspace_dir
            .join("memory")
            .join(format!("{}.md", today))
    }

    /// 判断是否为主会话
    fn is_main_session(&self, session_id: &SessionId) -> bool {
        crate::session_key::SessionKey::parse(session_id.as_str()).is_ok()
    }

    /// 读取文件内容（按优先级）
    ///
    /// 注入优先级：AGENTS.md > SOUL.md > MEMORY.md > memory/YYYY-MM-DD.md
    pub async fn get_injected_files(
        &self,
        session_id: &SessionId,
        is_main_session: Option<bool>,
    ) -> AgentResult<HashMap<String, String>> {
        let mut files = HashMap::new();
        let is_main_session = is_main_session.unwrap_or_else(|| self.is_main_session(session_id));

        // AGENTS.md - 总是注入
        if let Ok(content) = self.read_file("AGENTS.md").await {
            files.insert("AGENTS.md".to_string(), content);
        }

        // SOUL.md - 总是注入
        if let Ok(content) = self.read_file("SOUL.md").await {
            files.insert("SOUL.md".to_string(), content);
        }

        // MEMORY.md - 仅主会话注入
        if is_main_session {
            if let Ok(content) = self.read_file("MEMORY.md").await {
                files.insert("MEMORY.md".to_string(), content);
            }
        }

        // 今日 memory 文件
        let today_file = self.today_memory_file();
        if today_file.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&today_file).await {
                files.insert(
                    format!("memory/{}.md", Utc::now().format("%Y-%m-%d")),
                    content,
                );
            }
        }

        Ok(files)
    }

    /// 读取 workspace 中的文件
    async fn read_file(&self, name: &str) -> AgentResult<String> {
        let path = self.workspace_dir.join(name);
        tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| AgentError::Memory(format!("Failed to read {}: {}", name, e)))
    }

    /// 写入今日 memory 文件
    pub async fn append_to_today_memory(&self, content: &str) -> AgentResult<()> {
        let today_file = self.today_memory_file();

        // 确保目录存在
        if let Some(parent) = today_file.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AgentError::Memory(format!("Failed to create memory dir: {}", e)))?;
        }

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&today_file)
            .await
            .map_err(|e| AgentError::Memory(format!("Failed to open today memory: {}", e)))?;

        use tokio::io::AsyncWriteExt;
        file.write_all(content.as_bytes())
            .await
            .map_err(|e| AgentError::Memory(format!("Failed to append today memory: {}", e)))?;

        Ok(())
    }

    /// 保存会话状态
    pub async fn save_session_state(
        &self,
        session_key: &str,
        state: &SessionState,
    ) -> AgentResult<()> {
        let session_dir = self.session_dir(session_key);
        tokio::fs::create_dir_all(&session_dir)
            .await
            .map_err(|e| AgentError::Memory(format!("Failed to create session dir: {}", e)))?;

        let state_file = session_dir.join("state.json");
        let json = serde_json::to_string_pretty(state)
            .map_err(|e| AgentError::Memory(format!("Failed to serialize state: {}", e)))?;

        tokio::fs::write(&state_file, json)
            .await
            .map_err(|e| AgentError::Memory(format!("Failed to write state: {}", e)))?;

        Ok(())
    }

    /// 加载会话状态
    pub async fn load_session_state(&self, session_key: &str) -> AgentResult<Option<SessionState>> {
        let session_dir = self.session_dir(session_key);
        let state_file = session_dir.join("state.json");
        let legacy_state_file = session_dir.join("state.jsonl");

        let state_path = if state_file.exists() {
            state_file
        } else if legacy_state_file.exists() {
            legacy_state_file
        } else {
            return Ok(None);
        };

        let content = tokio::fs::read_to_string(&state_path)
            .await
            .map_err(|e| AgentError::Memory(format!("Failed to read state: {}", e)))?;

        let state = serde_json::from_str(&content)
            .map_err(|e| AgentError::Memory(format!("Failed to deserialize state: {}", e)))?;

        Ok(Some(state))
    }
}

/// 会话状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// 会话 ID
    pub session_id: String,
    /// 会话创建时间
    pub created_at: DateTime<Utc>,
    /// 最后活跃时间
    pub last_active: DateTime<Utc>,
    /// 消息数量
    pub message_count: usize,
    /// 额外数据
    pub metadata: HashMap<String, String>,
}

impl SessionState {
    /// 创建新的会话状态
    pub fn new(session_id: String) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            created_at: now,
            last_active: now,
            message_count: 0,
            metadata: HashMap::new(),
        }
    }

    /// 更新活跃时间
    pub fn touch(&mut self) {
        self.last_active = Utc::now();
    }

    /// 增加消息计数
    pub fn increment_messages(&mut self) {
        self.message_count += 1;
        self.last_active = Utc::now();
    }
}

/// Agent 管理器
///
/// 管理多个 Agent 的作用域。
pub struct AgentManager {
    /// 基础目录
    base_dir: PathBuf,
    /// Agent 作用域映射
    scopes: HashMap<String, Arc<AgentScope>>,
}

impl AgentManager {
    /// 创建新的 Agent 管理器
    pub fn new(base_dir: PathBuf) -> AgentResult<Self> {
        let base_str = base_dir.to_string_lossy().to_string();
        let expanded = shellexpand::tilde(&base_str);
        let base_dir = PathBuf::from(expanded.as_ref());

        Ok(Self {
            base_dir,
            scopes: HashMap::new(),
        })
    }

    /// 注册 Agent 作用域
    pub fn register_scope(&mut self, scope: Arc<AgentScope>) -> AgentResult<()> {
        let agent_id = scope.config().agent_id.clone();
        self.scopes.insert(agent_id, scope);
        Ok(())
    }

    /// 获取 Agent 作用域
    pub fn get_scope(&self, agent_id: &str) -> Option<&Arc<AgentScope>> {
        self.scopes.get(agent_id)
    }

    /// 获取默认 Agent 作用域
    pub fn get_default_scope(&self) -> Option<&Arc<AgentScope>> {
        self.scopes
            .values()
            .find(|s: &&Arc<AgentScope>| s.config().is_default)
    }

    /// 列出所有 Agent
    pub fn list_agents(&self) -> Vec<&Arc<AgentScope>> {
        self.scopes.values().collect()
    }

    /// 初始化所有 workspace
    pub async fn init_all_workspaces(&self) -> AgentResult<()> {
        for scope in self.scopes.values() {
            AgentScope::init_workspace(scope).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_scope() -> AgentScope {
        let tempdir = tempdir().unwrap();
        let workspace_dir = tempdir.into_path();

        AgentScope::new(AgentScopeConfig {
            agent_id: "test".to_string(),
            workspace_dir,
            display_name: Some("Test Agent".to_string()),
            is_default: false,
        })
        .unwrap()
    }

    #[tokio::test]
    async fn test_agent_scope_creation() {
        let scope = create_scope();
        assert_eq!(scope.config().agent_id, "test");
    }

    #[tokio::test]
    async fn test_get_injected_files_only_includes_memory_for_main_session() {
        let scope = create_scope();
        scope.init_workspace().await.unwrap();

        let memory_path = scope.workspace_dir().join("MEMORY.md");
        tokio::fs::write(&memory_path, "main memory").await.unwrap();

        let main_session = SessionId::from_string("dingtalk:user-1".to_string());
        let child_session = SessionId::from_string("session-123".to_string());

        let main_files = scope.get_injected_files(&main_session, None).await.unwrap();
        assert_eq!(
            main_files.get("MEMORY.md").map(String::as_str),
            Some("main memory")
        );

        let child_files = scope
            .get_injected_files(&child_session, None)
            .await
            .unwrap();
        assert!(!child_files.contains_key("MEMORY.md"));
    }

    #[tokio::test]
    async fn test_append_to_today_memory_appends() {
        let scope = create_scope();
        scope.init_workspace().await.unwrap();

        scope.append_to_today_memory("first\n").await.unwrap();
        scope.append_to_today_memory("second\n").await.unwrap();

        let content = tokio::fs::read_to_string(scope.today_memory_file())
            .await
            .unwrap();
        assert_eq!(content, "first\nsecond\n");
    }

    #[tokio::test]
    async fn test_session_state_round_trip_with_json_file() {
        let scope = create_scope();
        let mut state = SessionState::new("session-123".to_string());
        state
            .metadata
            .insert("current_agent".to_string(), "main".to_string());
        state.increment_messages();

        scope
            .save_session_state("dingtalk:user-1", &state)
            .await
            .unwrap();

        let loaded = scope
            .load_session_state("dingtalk:user-1")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(loaded.session_id, state.session_id);
        assert_eq!(loaded.message_count, state.message_count);
        assert_eq!(
            loaded.metadata.get("current_agent").map(String::as_str),
            Some("main")
        );
        assert!(scope
            .session_dir("dingtalk:user-1")
            .join("state.json")
            .exists());
    }

    #[tokio::test]
    async fn test_load_session_state_supports_legacy_jsonl_file() {
        let scope = create_scope();
        let session_dir = scope.session_dir("dingtalk:user-legacy");
        tokio::fs::create_dir_all(&session_dir).await.unwrap();

        let state = SessionState::new("legacy-session".to_string());
        let content = serde_json::to_string(&state).unwrap();
        tokio::fs::write(session_dir.join("state.jsonl"), content)
            .await
            .unwrap();

        let loaded = scope
            .load_session_state("dingtalk:user-legacy")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(loaded.session_id, "legacy-session");
    }

    #[tokio::test]
    async fn test_session_state() {
        let mut state = SessionState::new("session-123".to_string());
        assert_eq!(state.message_count, 0);

        state.increment_messages();
        assert_eq!(state.message_count, 1);
    }
}
