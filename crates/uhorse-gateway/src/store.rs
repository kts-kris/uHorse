//! # 内存存储服务
//!
//! 提供内存中的数据存储，用于 API Handler。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::api::types::*;

/// 通道状态
#[derive(Debug, Clone)]
pub struct ChannelStatus {
    pub channel_type: String,
    pub enabled: bool,
    pub running: bool,
    pub connected: bool,
    pub last_activity: Option<u64>,
    pub error: Option<String>,
}

impl ChannelStatus {
    pub fn new(channel_type: &str) -> Self {
        Self {
            channel_type: channel_type.to_string(),
            enabled: false,
            running: false,
            connected: false,
            last_activity: None,
            error: None,
        }
    }

    pub fn to_dto(&self) -> ChannelStatusDto {
        ChannelStatusDto {
            channel_type: self.channel_type.clone(),
            enabled: self.enabled,
            running: self.running,
            connected: self.connected,
            last_activity: self.last_activity.map(format_timestamp),
            error: self.error.clone(),
        }
    }
}

/// Agent 数据
#[derive(Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub is_default: bool,
    pub skills: Vec<String>,
    pub status: AgentStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

impl Agent {
    pub fn new(req: CreateAgentRequest) -> Self {
        let now = current_timestamp();
        Self {
            id: Uuid::new_v4().to_string(),
            name: req.name,
            description: req.description,
            system_prompt: req.system_prompt,
            model: req.model,
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            is_default: req.is_default,
            skills: req.skills,
            status: AgentStatus::Stopped,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn to_dto(&self) -> AgentDto {
        AgentDto {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            system_prompt: self.system_prompt.clone(),
            model: self.model.clone(),
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            is_default: self.is_default,
            skills: self.skills.clone(),
            created_at: format_timestamp(self.created_at),
            updated_at: format_timestamp(self.updated_at),
        }
    }
}

/// Agent 状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AgentStatus {
    Running,
    Stopped,
    Error,
}

/// Skill 数据
#[derive(Debug, Clone)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub parameters: Vec<SkillParameter>,
    pub manifest: String,
    pub agent_id: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl Skill {
    pub fn new(req: CreateSkillRequest) -> Self {
        let now = current_timestamp();
        Self {
            id: Uuid::new_v4().to_string(),
            name: req.name,
            description: req.description,
            version: req.version,
            author: req.author,
            tags: req.tags,
            parameters: req.parameters,
            manifest: req.manifest,
            agent_id: req.agent_id,
            created_at: now,
            updated_at: now,
        }
    }

    /// 从市场技能创建本地技能
    pub fn from_marketplace(market_skill: MarketplaceSkill, agent_id: Option<String>) -> Self {
        let now = current_timestamp();
        Self {
            id: Uuid::new_v4().to_string(),
            name: market_skill.name,
            description: market_skill.description,
            version: market_skill.version,
            author: Some(market_skill.author),
            tags: market_skill.tags,
            parameters: vec![],
            manifest: String::new(),
            agent_id,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn to_dto(&self) -> SkillDto {
        SkillDto {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            version: self.version.clone(),
            author: self.author.clone(),
            tags: self.tags.clone(),
            parameters: self.parameters.clone(),
            agent_id: self.agent_id.clone(),
            created_at: format_timestamp(self.created_at),
            updated_at: format_timestamp(self.updated_at),
        }
    }
}

/// Session 数据
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub agent_id: String,
    pub channel: String,
    pub user_id: String,
    pub status: SessionStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

impl Session {
    pub fn new(agent_id: String, channel: String, user_id: String) -> Self {
        let now = current_timestamp();
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id,
            channel,
            user_id,
            status: SessionStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn to_dto(&self) -> SessionDto {
        SessionDto {
            id: self.id.clone(),
            agent_id: self.agent_id.clone(),
            channel: self.channel.clone(),
            user_id: self.user_id.clone(),
            status: self.status,
            created_at: format_timestamp(self.created_at),
            updated_at: format_timestamp(self.updated_at),
        }
    }
}

/// Session 消息
#[derive(Debug, Clone)]
pub struct SessionMessage {
    pub id: String,
    pub session_id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: u64,
}

impl SessionMessage {
    pub fn new(session_id: String, role: MessageRole, content: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id,
            role,
            content,
            timestamp: current_timestamp(),
        }
    }

    pub fn to_dto(&self) -> SessionMessageDto {
        SessionMessageDto {
            id: self.id.clone(),
            role: self.role,
            content: self.content.clone(),
            timestamp: format_timestamp(self.timestamp),
        }
    }
}

/// 文件信息
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub agent_id: String,
    pub path: String,
    pub name: String,
    pub content: String,
    pub created_at: u64,
    pub updated_at: u64,
}

impl FileEntry {
    pub fn new(agent_id: String, path: String, content: String) -> Self {
        let name = path.split('/').last().unwrap_or(&path).to_string();
        let now = current_timestamp();
        Self {
            agent_id,
            path,
            name,
            content,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn to_info(&self) -> FileInfo {
        FileInfo {
            path: self.path.clone(),
            name: self.name.clone(),
            is_dir: false,
            size: Some(self.content.len() as u64),
            modified_at: Some(format_timestamp(self.updated_at)),
        }
    }
}

/// 内存存储
#[derive(Debug)]
pub struct MemoryStore {
    pub agents: Arc<RwLock<HashMap<String, Agent>>>,
    pub skills: Arc<RwLock<HashMap<String, Skill>>>,
    pub sessions: Arc<RwLock<HashMap<String, Session>>>,
    pub messages: Arc<RwLock<HashMap<String, Vec<SessionMessage>>>>,
    pub files: Arc<RwLock<HashMap<String, FileEntry>>>, // key: agent_id:path
    pub channels: Arc<RwLock<HashMap<String, ChannelStatus>>>,
}

impl Default for MemoryStore {
    fn default() -> Self {
        // 初始化所有支持的通道
        let channels = HashMap::from([
            ("telegram".to_string(), ChannelStatus::new("telegram")),
            ("dingtalk".to_string(), ChannelStatus::new("dingtalk")),
            ("feishu".to_string(), ChannelStatus::new("feishu")),
            ("wework".to_string(), ChannelStatus::new("wework")),
            ("slack".to_string(), ChannelStatus::new("slack")),
            ("discord".to_string(), ChannelStatus::new("discord")),
            ("whatsapp".to_string(), ChannelStatus::new("whatsapp")),
        ]);

        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            skills: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(HashMap::new())),
            files: Arc::new(RwLock::new(HashMap::new())),
            channels: Arc::new(RwLock::new(channels)),
        }
    }
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    // Agent 操作
    pub async fn list_agents(&self, page: u32, per_page: u32) -> (Vec<Agent>, u64) {
        let agents = self.agents.read().await;
        let total = agents.len() as u64;
        let skip = ((page - 1) * per_page) as usize;
        let items: Vec<Agent> = agents
            .values()
            .skip(skip)
            .take(per_page as usize)
            .cloned()
            .collect();
        (items, total)
    }

    pub async fn get_agent(&self, id: &str) -> Option<Agent> {
        self.agents.read().await.get(id).cloned()
    }

    pub async fn create_agent(&self, agent: Agent) -> Agent {
        let id = agent.id.clone();
        self.agents.write().await.insert(id.clone(), agent.clone());
        agent
    }

    pub async fn update_agent(&self, id: &str, req: UpdateAgentRequest) -> Option<Agent> {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(id) {
            if let Some(name) = req.name {
                agent.name = name;
            }
            if let Some(description) = req.description {
                agent.description = Some(description);
            }
            if let Some(system_prompt) = req.system_prompt {
                agent.system_prompt = Some(system_prompt);
            }
            if let Some(model) = req.model {
                agent.model = Some(model);
            }
            if let Some(temperature) = req.temperature {
                agent.temperature = Some(temperature);
            }
            if let Some(max_tokens) = req.max_tokens {
                agent.max_tokens = Some(max_tokens);
            }
            if let Some(is_default) = req.is_default {
                agent.is_default = is_default;
            }
            if let Some(skills) = req.skills {
                agent.skills = skills;
            }
            agent.updated_at = current_timestamp();
            return Some(agent.clone());
        }
        None
    }

    pub async fn delete_agent(&self, id: &str) -> bool {
        self.agents.write().await.remove(id).is_some()
    }

    // Skill 操作
    pub async fn list_skills(&self, page: u32, per_page: u32) -> (Vec<Skill>, u64) {
        let skills = self.skills.read().await;
        let total = skills.len() as u64;
        let skip = ((page - 1) * per_page) as usize;
        let items: Vec<Skill> = skills
            .values()
            .skip(skip)
            .take(per_page as usize)
            .cloned()
            .collect();
        (items, total)
    }

    pub async fn get_skill(&self, id: &str) -> Option<Skill> {
        self.skills.read().await.get(id).cloned()
    }

    pub async fn create_skill(&self, skill: Skill) -> Skill {
        let id = skill.id.clone();
        self.skills.write().await.insert(id.clone(), skill.clone());
        skill
    }

    pub async fn update_skill(&self, id: &str, req: UpdateSkillRequest) -> Option<Skill> {
        let mut skills = self.skills.write().await;
        if let Some(skill) = skills.get_mut(id) {
            if let Some(name) = req.name {
                skill.name = name;
            }
            if let Some(description) = req.description {
                skill.description = description;
            }
            if let Some(version) = req.version {
                skill.version = version;
            }
            if let Some(tags) = req.tags {
                skill.tags = tags;
            }
            if let Some(parameters) = req.parameters {
                skill.parameters = parameters;
            }
            if let Some(manifest) = req.manifest {
                skill.manifest = manifest;
            }
            skill.updated_at = current_timestamp();
            return Some(skill.clone());
        }
        None
    }

    pub async fn delete_skill(&self, id: &str) -> bool {
        self.skills.write().await.remove(id).is_some()
    }

    // Session 操作
    pub async fn list_sessions(&self, page: u32, per_page: u32) -> (Vec<Session>, u64) {
        let sessions = self.sessions.read().await;
        let total = sessions.len() as u64;
        let skip = ((page - 1) * per_page) as usize;
        let items: Vec<Session> = sessions
            .values()
            .skip(skip)
            .take(per_page as usize)
            .cloned()
            .collect();
        (items, total)
    }

    pub async fn get_session(&self, id: &str) -> Option<Session> {
        self.sessions.read().await.get(id).cloned()
    }

    pub async fn delete_session(&self, id: &str) -> bool {
        let removed = self.sessions.write().await.remove(id).is_some();
        self.messages.write().await.remove(id);
        removed
    }

    pub async fn get_session_messages(
        &self,
        session_id: &str,
        page: u32,
        per_page: u32,
    ) -> (Vec<SessionMessage>, u64) {
        let messages = self.messages.read().await;
        let session_messages = messages.get(session_id).cloned().unwrap_or_default();
        let total = session_messages.len() as u64;
        let skip = ((page - 1) * per_page) as usize;
        let items: Vec<SessionMessage> = session_messages
            .into_iter()
            .skip(skip)
            .take(per_page as usize)
            .collect();
        (items, total)
    }

    // 文件操作
    pub async fn list_files(&self, agent_id: &str) -> Vec<FileInfo> {
        let files = self.files.read().await;
        files
            .values()
            .filter(|f| f.agent_id == agent_id)
            .map(|f| f.to_info())
            .collect()
    }

    pub async fn get_file(&self, agent_id: &str, path: &str) -> Option<FileEntry> {
        let key = format!("{}:{}", agent_id, path);
        self.files.read().await.get(&key).cloned()
    }

    pub async fn save_file(&self, agent_id: &str, path: &str, content: String) -> FileEntry {
        let key = format!("{}:{}", agent_id, path);
        let file = FileEntry::new(agent_id.to_string(), path.to_string(), content);
        self.files.write().await.insert(key, file.clone());
        file
    }

    pub async fn delete_file(&self, agent_id: &str, path: &str) -> bool {
        let key = format!("{}:{}", agent_id, path);
        self.files.write().await.remove(&key).is_some()
    }

    // 通道操作
    pub async fn list_channels(&self) -> Vec<ChannelStatusDto> {
        let channels = self.channels.read().await;
        channels.values().map(|c| c.to_dto()).collect()
    }

    pub async fn get_channel_status(&self, channel_type: &str) -> Option<ChannelStatusDto> {
        let channels = self.channels.read().await;
        channels.get(channel_type).map(|c| c.to_dto())
    }

    pub async fn set_channel_enabled(&self, channel_type: &str, enabled: bool) -> ChannelStatusDto {
        let mut channels = self.channels.write().await;
        if let Some(status) = channels.get_mut(channel_type) {
            status.enabled = enabled;
            status.last_activity = Some(current_timestamp());
            return status.to_dto();
        }
        // 如果通道不存在，创建新的
        let mut status = ChannelStatus::new(channel_type);
        status.enabled = enabled;
        status.last_activity = Some(current_timestamp());
        let dto = status.to_dto();
        channels.insert(channel_type.to_string(), status);
        dto
    }
}

// 辅助函数
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn format_timestamp(ts: u64) -> String {
    // ISO 8601 格式
    let datetime =
        chrono::DateTime::from_timestamp(ts as i64, 0).unwrap_or_else(|| chrono::Utc::now());
    datetime.to_rfc3339()
}
