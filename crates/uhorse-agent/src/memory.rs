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

use crate::error::{AgentError, AgentResult};
use serde::{Deserialize, Serialize};
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
    async fn store_kv(
        &self,
        session_id: &SessionId,
        key: &str,
        value: &str,
    ) -> AgentResult<()>;

    /// 获取键值对
    async fn get_kv(
        &self,
        session_id: &SessionId,
        key: &str,
    ) -> AgentResult<Option<String>>;
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

    /// 获取会话目录
    fn session_dir(&self, session_id: &SessionId) -> PathBuf {
        self.workspace_dir.join("sessions").join(session_id.as_str())
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
            tokio::fs::read_to_string(&history_path).await.unwrap_or_default()
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
    async fn store_kv(
        &self,
        session_id: &SessionId,
        key: &str,
        value: &str,
    ) -> AgentResult<()> {
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
    async fn get_kv(
        &self,
        session_id: &SessionId,
        key: &str,
    ) -> AgentResult<Option<String>> {
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
