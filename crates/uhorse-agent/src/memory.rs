//! # Memory - 记忆系统
//!
//! OpenClaw 风格的记忆系统，采用"记忆即文件"的设计。
//!
//! ## 目录结构
//!
//! ```text
//! ~/.uhorse/workspace/
//! ├── MEMORY.md         # 长期记忆
//! ├── SOUL.md           # 性格设定
//! ├── USER.md           # 用户偏好
//! ├── HEARTBEAT.md      # 状态记录
//! └── TODO.md           # 任务列表
//! ```

use crate::error::AgentResult;
use crate::session_key::{scope_layer_from_scope, SessionKey, SessionNamespace};
use std::collections::HashMap;
use std::path::PathBuf;
use uhorse_core::SessionId;

/// 记忆存储接口
#[async_trait::async_trait]
pub trait MemoryStore: Send + Sync {
    /// 存储消息
    async fn store_message(
        &self,
        session_id: &SessionId,
        user_message: &str,
        assistant_message: &str,
    ) -> AgentResult<()>;

    /// 获取上下文
    async fn get_context(&self, session_id: &SessionId) -> AgentResult<String>;

    /// 存储键值对
    async fn store_kv(&self, session_id: &SessionId, key: &str, value: &str) -> AgentResult<()>;

    /// 获取键值对
    async fn get_kv(&self, session_id: &SessionId, key: &str) -> AgentResult<Option<String>>;
}

/// 文件系统记忆存储
///
/// 实现 OpenClaw 风格的"记忆即文件"设计。
pub struct FileMemory {
    /// 工作空间目录
    workspace_dir: PathBuf,
}

impl FileMemory {
    /// 创建新的文件记忆
    pub fn new(workspace_dir: PathBuf) -> Self {
        Self { workspace_dir }
    }

    /// 返回工作空间目录。
    pub fn workspace_dir(&self) -> &PathBuf {
        &self.workspace_dir
    }

    /// 获取会话目录
    fn session_dir(&self, session_id: &SessionId) -> PathBuf {
        self.workspace_dir
            .join("sessions")
            .join(session_id.as_str())
    }

    /// 初始化工作空间
    pub async fn init_workspace(&self) -> AgentResult<()> {
        // 创建工作空间目录
        tokio::fs::create_dir_all(&self.workspace_dir).await?;

        // 创建核心记忆文件
        let memory_md = self.workspace_dir.join("MEMORY.md");
        if !memory_md.exists() {
            tokio::fs::write(
                &memory_md,
                "# Long-term Memory\n\nThis file stores long-term memories and important information.\n",
            )
            .await?;
        }

        let soul_md = self.workspace_dir.join("SOUL.md");
        if !soul_md.exists() {
            tokio::fs::write(
                &soul_md,
                "# Soul / Personality\n\nThis file defines the agent's personality and behavior patterns.\n",
            )
            .await?;
        }

        let user_md = self.workspace_dir.join("USER.md");
        if !user_md.exists() {
            tokio::fs::write(
                &user_md,
                "# User Preferences\n\nThis file stores user preferences and settings.\n",
            )
            .await?;
        }

        Ok(())
    }

    /// 读取 MEMORY.md
    pub async fn read_memory_md(&self) -> AgentResult<String> {
        let path = self.workspace_dir.join("MEMORY.md");
        Ok(tokio::fs::read_to_string(&path).await?)
    }

    /// 读取 SOUL.md
    pub async fn read_soul_md(&self) -> AgentResult<String> {
        let path = self.workspace_dir.join("SOUL.md");
        Ok(tokio::fs::read_to_string(&path).await?)
    }

    /// 读取 USER.md
    pub async fn read_user_md(&self) -> AgentResult<String> {
        let path = self.workspace_dir.join("USER.md");
        Ok(tokio::fs::read_to_string(&path).await?)
    }
}

/// 分层记忆存储。
///
/// 在保留 `FileMemory` 作为底层文件模型的前提下，基于 `SessionKey`
/// 推导 `global / tenant / user / session` 四层视图：
/// - `store_message` 继续写入 session history；
/// - `store_kv` 默认写入 user scope；
/// - `get_context` 按 namespace 的共享链聚合。
#[derive(Clone)]
pub struct LayeredMemoryStore {
    workspace_dir: PathBuf,
}

impl LayeredMemoryStore {
    /// 创建新的分层记忆存储。
    pub fn new(workspace_dir: PathBuf) -> Self {
        Self { workspace_dir }
    }

    /// 初始化全局工作空间。
    pub async fn init_workspace(&self) -> AgentResult<()> {
        self.global_memory().init_workspace().await
    }

    fn global_memory(&self) -> FileMemory {
        FileMemory::new(self.workspace_dir.clone())
    }

    fn scoped_memory(&self, scope: &str) -> FileMemory {
        match scope_layer_from_scope(scope) {
            "tenant" => FileMemory::new(self.workspace_dir.join("tenants").join(scope)),
            "enterprise" => FileMemory::new(self.workspace_dir.join("enterprises").join(scope)),
            "department" => FileMemory::new(self.workspace_dir.join("departments").join(scope)),
            "role" => FileMemory::new(self.workspace_dir.join("roles").join(scope)),
            "user" => FileMemory::new(self.workspace_dir.join("users").join(scope)),
            _ => self.global_memory(),
        }
    }

    fn namespace_for_session(&self, session_id: &SessionId) -> Option<SessionNamespace> {
        SessionKey::parse(session_id.as_str())
            .ok()
            .map(|session_key| session_key.namespace())
    }

    /// 使用显式 namespace 获取上下文。
    pub async fn get_context_for_namespace(
        &self,
        session_id: &SessionId,
        namespace: &SessionNamespace,
    ) -> AgentResult<String> {
        let mut sections = Vec::new();
        for scope in namespace.memory_context_chain() {
            let memory = if scope == namespace.global {
                Self::read_scope_memory(&self.global_memory()).await
            } else if scope == namespace.session {
                String::new()
            } else {
                Self::read_scope_memory(&self.scoped_memory(&scope)).await
            };
            if !memory.is_empty() {
                sections.push(format!(
                    "=== {} Memory ===\n{}",
                    Self::named_scope_label(&scope),
                    memory
                ));
            }
        }

        let session_history = Self::read_session_history(&self.global_memory(), session_id).await;
        if !session_history.is_empty() {
            sections.push(format!("=== Session History ===\n{}", session_history));
        }

        Ok(sections.join("\n\n"))
    }

    fn scope_session_id(scope: &str) -> SessionId {
        SessionId::from_string(scope.to_string())
    }

    fn named_scope_label(scope: &str) -> String {
        match scope_layer_from_scope(scope) {
            "global" => "Global".to_string(),
            "tenant" => "Tenant".to_string(),
            "enterprise" => "Enterprise".to_string(),
            "department" => "Department".to_string(),
            "role" => "Role".to_string(),
            "user" => "User".to_string(),
            other => {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => "Scope".to_string(),
                }
            }
        }
    }

    async fn read_scope_memory(store: &FileMemory) -> String {
        store.read_memory_md().await.unwrap_or_default()
    }

    async fn read_session_history(store: &FileMemory, session_id: &SessionId) -> String {
        let history_path = store.session_dir(session_id).join("history.md");
        if history_path.exists() {
            tokio::fs::read_to_string(history_path)
                .await
                .unwrap_or_default()
        } else {
            String::new()
        }
    }
}

#[async_trait::async_trait]
impl MemoryStore for LayeredMemoryStore {
    async fn store_message(
        &self,
        session_id: &SessionId,
        user_message: &str,
        assistant_message: &str,
    ) -> AgentResult<()> {
        self.global_memory()
            .store_message(session_id, user_message, assistant_message)
            .await
    }

    async fn get_context(&self, session_id: &SessionId) -> AgentResult<String> {
        let Some(namespace) = self.namespace_for_session(session_id) else {
            return self.global_memory().get_context(session_id).await;
        };

        self.get_context_for_namespace(session_id, &namespace).await
    }

    async fn store_kv(&self, session_id: &SessionId, key: &str, value: &str) -> AgentResult<()> {
        let Some(namespace) = self.namespace_for_session(session_id) else {
            return self.global_memory().store_kv(session_id, key, value).await;
        };

        self.scoped_memory(&namespace.user)
            .store_kv(&Self::scope_session_id(&namespace.user), key, value)
            .await
    }

    async fn get_kv(&self, session_id: &SessionId, key: &str) -> AgentResult<Option<String>> {
        let Some(namespace) = self.namespace_for_session(session_id) else {
            return self.global_memory().get_kv(session_id, key).await;
        };

        for scope in namespace.visibility_chain() {
            if scope == namespace.global {
                if let Some(value) = self
                    .global_memory()
                    .get_kv(&Self::scope_session_id(&scope), key)
                    .await?
                {
                    return Ok(Some(value));
                }
            } else if let Some(value) = self
                .scoped_memory(&scope)
                .get_kv(&Self::scope_session_id(&scope), key)
                .await?
            {
                return Ok(Some(value));
            }
        }

        self.global_memory().get_kv(session_id, key).await
    }
}

#[async_trait::async_trait]
impl MemoryStore for FileMemory {
    /// 存储消息
    async fn store_message(
        &self,
        session_id: &SessionId,
        user_message: &str,
        assistant_message: &str,
    ) -> AgentResult<()> {
        let session_dir = self.session_dir(session_id);
        tokio::fs::create_dir_all(&session_dir).await?;

        // 追加到会话历史文件
        let history_path = session_dir.join("history.md");
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");

        let entry = format!(
            "## {}\n\n**User:** {}\n\n**Assistant:** {}\n\n",
            timestamp, user_message, assistant_message
        );

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&history_path)
            .await?;

        use tokio::io::AsyncWriteExt;
        file.write_all(entry.as_bytes()).await?;

        Ok(())
    }

    /// 获取上下文
    async fn get_context(&self, session_id: &SessionId) -> AgentResult<String> {
        let session_dir = self.session_dir(session_id);

        // 读取全局记忆
        let global_memory = self.read_memory_md().await.unwrap_or_default();

        // 读取会话历史
        let history_path = session_dir.join("history.md");
        let session_history = if history_path.exists() {
            tokio::fs::read_to_string(&history_path)
                .await
                .unwrap_or_default()
        } else {
            String::new()
        };

        // 组合上下文
        if global_memory.is_empty() && session_history.is_empty() {
            Ok(String::new())
        } else {
            Ok(format!(
                "=== Global Memory ===\n{}\n\n=== Session History ===\n{}",
                global_memory, session_history
            ))
        }
    }

    /// 存储键值对
    async fn store_kv(&self, session_id: &SessionId, key: &str, value: &str) -> AgentResult<()> {
        let session_dir = self.session_dir(session_id);
        tokio::fs::create_dir_all(&session_dir).await?;

        let kv_path = session_dir.join("kv.json");

        // 读取现有 KV
        let mut kvs: HashMap<String, String> = if kv_path.exists() {
            let content = tokio::fs::read_to_string(&kv_path).await?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        // 更新
        kvs.insert(key.to_string(), value.to_string());

        // 写回
        let content = serde_json::to_string_pretty(&kvs)?;
        tokio::fs::write(&kv_path, content).await?;

        Ok(())
    }

    /// 获取键值对
    async fn get_kv(&self, session_id: &SessionId, key: &str) -> AgentResult<Option<String>> {
        let session_dir = self.session_dir(session_id);
        let kv_path = session_dir.join("kv.json");

        if !kv_path.exists() {
            return Ok(None);
        }

        let content = tokio::fs::read_to_string(&kv_path).await?;
        let kvs: HashMap<String, String> = serde_json::from_str(&content)?;

        Ok(kvs.get(key).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_memory() -> FileMemory {
        let workspace_dir = tempdir().unwrap().keep();
        FileMemory::new(workspace_dir)
    }

    fn create_layered_memory() -> LayeredMemoryStore {
        let workspace_dir = tempdir().unwrap().keep();
        LayeredMemoryStore::new(workspace_dir)
    }

    #[tokio::test]
    async fn test_store_message_appends_history() {
        let memory = create_memory();
        let session_id = SessionId::from_string("session-1".to_string());

        memory
            .store_message(&session_id, "hello", "world")
            .await
            .unwrap();
        memory
            .store_message(&session_id, "again", "done")
            .await
            .unwrap();

        let history = tokio::fs::read_to_string(memory.session_dir(&session_id).join("history.md"))
            .await
            .unwrap();

        assert!(history.contains("**User:** hello"));
        assert!(history.contains("**Assistant:** world"));
        assert!(history.contains("**User:** again"));
        assert!(history.contains("**Assistant:** done"));
    }

    #[tokio::test]
    async fn test_get_context_combines_global_memory_and_session_history() {
        let memory = create_memory();
        memory.init_workspace().await.unwrap();

        tokio::fs::write(memory.workspace_dir.join("MEMORY.md"), "global facts")
            .await
            .unwrap();

        let session_id = SessionId::from_string("session-1".to_string());
        memory
            .store_message(&session_id, "hello", "world")
            .await
            .unwrap();

        let context = memory.get_context(&session_id).await.unwrap();
        assert!(context.contains("=== Global Memory ==="));
        assert!(context.contains("global facts"));
        assert!(context.contains("=== Session History ==="));
        assert!(context.contains("**User:** hello"));
    }

    #[tokio::test]
    async fn test_store_and_get_kv_round_trip() {
        let memory = create_memory();
        let session_id = SessionId::from_string("session-1".to_string());

        memory.store_kv(&session_id, "agent", "main").await.unwrap();
        let value = memory.get_kv(&session_id, "agent").await.unwrap();

        assert_eq!(value.as_deref(), Some("main"));
    }

    #[tokio::test]
    async fn test_layered_memory_get_context_combines_scopes() {
        let memory = create_layered_memory();
        memory.init_workspace().await.unwrap();

        tokio::fs::write(memory.workspace_dir.join("MEMORY.md"), "global facts")
            .await
            .unwrap();
        tokio::fs::create_dir_all(
            memory
                .workspace_dir
                .join("tenants")
                .join("tenant:dingtalk:corp-1"),
        )
        .await
        .unwrap();
        tokio::fs::write(
            memory
                .workspace_dir
                .join("tenants")
                .join("tenant:dingtalk:corp-1")
                .join("MEMORY.md"),
            "tenant facts",
        )
        .await
        .unwrap();
        tokio::fs::create_dir_all(
            memory
                .workspace_dir
                .join("users")
                .join("user:dingtalk:user-1"),
        )
        .await
        .unwrap();
        tokio::fs::write(
            memory
                .workspace_dir
                .join("users")
                .join("user:dingtalk:user-1")
                .join("MEMORY.md"),
            "user facts",
        )
        .await
        .unwrap();

        let session_id = SessionId::from_string("dingtalk:user-1:corp-1".to_string());
        memory
            .store_message(&session_id, "hello", "world")
            .await
            .unwrap();

        let context = memory.get_context(&session_id).await.unwrap();
        assert!(context.contains("=== Global Memory ==="));
        assert!(context.contains("global facts"));
        assert!(context.contains("=== Tenant Memory ==="));
        assert!(context.contains("tenant facts"));
        assert!(context.contains("=== User Memory ==="));
        assert!(context.contains("user facts"));
        assert!(context.contains("=== Session History ==="));
        assert!(context.contains("**User:** hello"));
    }

    #[tokio::test]
    async fn test_layered_memory_get_context_for_expanded_namespace() {
        let memory = create_layered_memory();
        memory.init_workspace().await.unwrap();

        tokio::fs::write(memory.workspace_dir.join("MEMORY.md"), "global facts")
            .await
            .unwrap();
        for (dir, scope, content) in [
            ("tenants", "tenant:dingtalk:corp-1", "tenant facts"),
            ("enterprises", "enterprise:org-1", "enterprise facts"),
            ("departments", "department:org-1:sales", "department facts"),
            ("roles", "role:org-1:manager", "role facts"),
            ("users", "user:dingtalk:user-1", "user facts"),
        ] {
            let scope_dir = memory.workspace_dir.join(dir).join(scope);
            tokio::fs::create_dir_all(&scope_dir).await.unwrap();
            tokio::fs::write(scope_dir.join("MEMORY.md"), content)
                .await
                .unwrap();
        }

        let session_id = SessionId::from_string("dingtalk:user-1:corp-1".to_string());
        memory
            .store_message(&session_id, "hello", "world")
            .await
            .unwrap();

        let namespace = SessionKey::with_team("dingtalk", "user-1", "corp-1")
            .namespace_with_access_context(Some(&crate::session_key::AccessContext {
                tenant: None,
                enterprise: Some("enterprise:org-1".to_string()),
                department: Some("department:org-1:sales".to_string()),
                roles: vec!["role:org-1:manager".to_string()],
            }));
        let context = memory
            .get_context_for_namespace(&session_id, &namespace)
            .await
            .unwrap();

        assert!(context.contains("=== Tenant Memory ==="));
        assert!(context.contains("tenant facts"));
        assert!(context.contains("=== Enterprise Memory ==="));
        assert!(context.contains("enterprise facts"));
        assert!(context.contains("=== Department Memory ==="));
        assert!(context.contains("department facts"));
        assert!(context.contains("=== Role Memory ==="));
        assert!(context.contains("role facts"));
        assert!(context.contains("=== User Memory ==="));
        assert!(context.contains("user facts"));
    }

    #[tokio::test]
    async fn test_layered_memory_store_kv_defaults_to_user_scope() {
        let memory = create_layered_memory();
        let session_id = SessionId::from_string("dingtalk:user-1:corp-1".to_string());

        memory.store_kv(&session_id, "theme", "dark").await.unwrap();
        let value = memory.get_kv(&session_id, "theme").await.unwrap();

        assert_eq!(value.as_deref(), Some("dark"));
        let kv_path = memory
            .workspace_dir
            .join("users")
            .join("user:dingtalk:user-1")
            .join("sessions")
            .join("user:dingtalk:user-1")
            .join("kv.json");
        assert!(kv_path.exists());
    }
}
