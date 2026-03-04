# uHorse v1.1 Implementation Plan

## Overview

**Version**: v1.1
**Timeline**: 5 weeks
**Priority**: High

## Goals

1. **Web 管理界面** - 企业级管理仪表盘
2. **更多 LLM 提供商** - 支持 Anthropic、Gemini、本地 LLM
3. **技能市场** - 技能发现、安装、分享

## Phase 1: API Foundation (Backend)

### 1.1 API Types & DTOs

**File**: `crates/uhorse-gateway/src/api/types.rs`

```rust
use serde::{Deserialize, Serialize};

// === 通用响应 ===
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ApiError>,
}

#[derive(Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

// === 分页 ===
#[derive(Deserialize)]
pub struct PaginationQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Serialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
}

// === Agent DTOs ===
#[derive(Serialize, Deserialize)]
pub struct AgentDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub is_default: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateAgentRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub is_default: Option<bool>,
}

// === Skill DTOs ===
#[derive(Serialize, Deserialize)]
pub struct SkillDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub parameters: Vec<SkillParameter>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct SkillParameter {
    pub name: String,
    pub description: String,
    pub parameter_type: String,
    pub required: bool,
}

// === Session DTOs ===
#[derive(Serialize, Deserialize)]
pub struct SessionDto {
    pub id: String,
    pub agent_id: String,
    pub channel: String,
    pub user_id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct SessionMessageDto {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
}

// === File DTOs ===
#[derive(Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
}

// === Auth DTOs ===
#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

// === Marketplace DTOs ===
#[derive(Serialize, Deserialize)]
pub struct MarketplaceSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub downloads: u64,
    pub rating: f32,
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub tags: Option<Vec<String>>,
    pub sort: Option<String>,
}
```

### 1.2 API Routes

**File**: `crates/uhorse-gateway/src/api/routes.rs`

```rust
use axum::{
    extract::Path,
    routing::{get, post, put, delete, Router,
};
use crate::api::types::*;
use crate::api::handlers::*;

pub fn api_routes() -> Router {
    Router::new()
        // === Health ===
        .route("/health/live", get(health_live))
        .route("/health/ready", get(health_ready))
        // === Auth ===
        .route("/api/v1/auth/login", post(auth_login))
        .route("/api/v1/auth/logout", post(auth_logout))
        .route("/api/v1/auth/refresh", post(auth_refresh))
        // === Agents ===
        .route("/api/v1/agents", get(list_agents).post(create_agent))
        .route("/api/v1/agents/:id", get(get_agent).put(update_agent).delete(delete_agent))
        // === Skills ===
        .route("/api/v1/skills", get(list_skills).post(create_skill))
        .route("/api/v1/skills/:id", get(get_skill).put(update_skill).delete(delete_skill))
        // === Sessions ===
        .route("/api/v1/sessions", get(list_sessions))
        .route("/api/v1/sessions/:id", get(get_session).delete(delete_session))
        .route("/api/v1/sessions/:id/messages", get(get_session_messages))
        // === Files ===
        .route("/api/v1/files/:agent_id", get(list_files))
        .route("/api/v1/files/:agent_id/*path", get(get_file).put(update_file))
        // === Marketplace ===
        .route("/api/v1/marketplace/search", get(search_marketplace))
        .route("/api/v1/marketplace/skills/:id", get(get_marketplace_skill))
        .route("/api/v1/marketplace/install/:id", post(install_marketplace_skill))
```

### 1.3 API Handlers

**File**: `crates/uhorse-gateway/src/api/handlers.rs`

```rust
// Health handlers
pub async fn health_live() -> &'static str {
    "\"alive\""
}

pub async fn health_ready() -> Json<serde_json::Value> {
    // Check database, channels, etc.
    json!({"status": "ready"})
}

// Auth handlers
pub async fn auth_login(
    State: AppState,
    Json(req): LoginRequest,
) -> Result<Json<TokenResponse>, StatusCode> {
    // Validate credentials
    let token = state.auth_service.login(&req.username, &req.password).await?;
    Ok(Json(token))
}

// Agent handlers
pub async fn list_agents(
    State: AppState,
) -> Result<Json<Vec<AgentDto>>, StatusCode> {
    let agents = state.agent_service.list().await?;
    Ok(Json(agents))
}

pub async fn create_agent(
    State: AppState,
    Json(req): CreateAgentRequest,
) -> Result<Json<AgentDto>, StatusCode> {
    let agent = state.agent_service.create(req).await?;
    Ok(Json(agent))
}

pub async fn get_agent(
    State: AppState,
    Path(id): String,
) -> Result<Json<AgentDto>, StatusCode> {
    let agent = state.agent_service.get(&id).await?;
    Ok(Json(agent))
}

// Skill handlers
pub async fn list_skills(
    State: AppState,
    Query(params): PaginationQuery,
) -> Result<Json<PaginatedResponse<SkillDto>>, StatusCode> {
    let skills = state.skill_service.list(params).await?;
    Ok(Json(skills))
}

// Session handlers
pub async fn list_sessions(
    State: AppState,
    Query(params): PaginationQuery,
) -> Result<Json<PaginatedResponse<SessionDto>>, StatusCode> {
    let sessions = state.session_service.list(params).await?;
    Ok(Json(sessions))
}

// File handlers
pub async fn list_files(
    State: AppState,
    Path(agent_id): String,
) -> Result<Json<Vec<FileInfo>>, StatusCode> {
    let files = state.file_service.list(&agent_id).await?;
    Ok(Json(files))
}

pub async fn get_file(
    State: AppState,
    Path(agent_id): String,
    Path(path): String,
) -> Result<String, StatusCode> {
    let content = state.file_service.read(&agent_id, &path).await?;
    Ok(content)
}

pub async fn update_file(
    State: AppState,
    Path(agent_id): String,
    Path(path): String,
    body: String,
) -> Result<StatusCode, StatusCode> {
    state.file_service.write(&agent_id, &path, &body).await?;
    Ok(StatusCode::NO_CONTENT)
}

// Marketplace handlers
pub async fn search_marketplace(
    State: AppState,
    Query(query): SearchQuery,
) -> Result<Json<Vec<MarketplaceSkill>>, StatusCode> {
    let skills = state.marketplace_service.search(&query).await?;
    Ok(Json(skills))
}

pub async fn install_marketplace_skill(
    State: AppState,
    Path(id): String,
) -> Result<Json<SkillDto>, StatusCode> {
    let skill = state.marketplace_service.install(&id).await?;
    Ok(Json(skill))
}
```

## Phase 2: LLM Providers

### 2.1 LLM Client Trait Refactor

**File**: `crates/uhorse-llm/src/client.rs`

```rust
use crate::config::{LLMConfig, LLMProvider};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// LLM 客户端 Trait
#[async_trait]
pub trait LLMClient: Send + Sync {
    /// 发送聊天请求
    async fn chat_completion(&self, messages: Vec<ChatMessage>) -> Result<String>;

    /// 流式聊天请求
    async fn chat_completion_stream(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<Box<dyn futures::Stream<Item = String> + Unpin>> {
}

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".to_string(), content: content.into() }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".to_string(), content: content.into() }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".to_string(), content: content.into() }
    }
}

/// 客户端工厂
pub struct LLMClientFactory;

impl LLMClientFactory {
    pub fn create(config: LLMConfig) -> Result<Box<dyn LLMClient>> {
        match config.provider {
            LLMProvider::OpenAI | LLMProvider::AzureOpenAI => {
                Ok(Box::new(OpenAIClient::new(config)?))
            }
            LLMProvider::Anthropic => {
                Ok(Box::new(AnthropicClient::new(config)?))
            }
            LLMProvider::Gemini => {
                Ok(Box::new(GeminiClient::new(config)?))
            }
            LLMProvider::Custom(name) => {
                // Custom providers use OpenAI-compatible API
                Ok(Box::new(OpenAIClient::new(config)?))
            }
        }
    }
}
```

### 2.2 Anthropic Client

**File**: `crates/uhorse-llm/src/anthropic.rs`

```rust
use crate::client::{ChatMessage, LLMClient};
use crate::config::LLMConfig;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

pub struct AnthropicClient {
    config: LLMConfig,
    client: Client,
}

impl AnthropicClient {
    pub fn new(config: LLMConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;
        Ok(Self { config, client })
    }

    fn build_url(&self) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        format!("{}/v1/messages", base)
    }
}

#[async_trait]
impl LLMClient for AnthropicClient {
    async fn chat_completion(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let url = self.build_url();

        // 转换消息格式为 Anthropic 格式
        let (system, conversation) = self.convert_messages(&messages);

        let body = json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "system": system,
            "messages": conversation,
        });

        let response = self.client
            .post(&url)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(anyhow::anyhow!("Anthropic API error: {}", error));
        }

        let result: AnthropicResponse = response.json().await?;
        Ok(result.content.first().text)
    }

    fn convert_messages(&self, messages: &[ChatMessage]) -> (Option<String>, Vec<AnthropicMessage>) {
        let mut system = None;
        let mut conversation = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => system = Some(msg.content.clone()),
                _ => conversation.push(AnthropicMessage {
                    role: msg.role.clone(),
                    content: msg.content.clone(),
                }),
            }
        }

        (system, conversation)
    }
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}
```

### 2.3 Gemini Client

**File**: `crates/uhorse-llm/src/gemini.rs`

```rust
use crate::client::{ChatMessage, LLMClient};
use crate::config::LLMConfig;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

pub struct GeminiClient {
    config: LLMConfig,
    client: Client,
}

impl GeminiClient {
    pub fn new(config: LLMConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;
        Ok(Self { config, client })
    }

    fn build_url(&self) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            base, self.config.model, urlencoding::encode(&self.config.api_key)
        )
    }
}

#[async_trait]
impl LLMClient for GeminiClient {
    async fn chat_completion(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let url = self.build_url();

        let contents = self.convert_messages(&messages);

        let body = json!({
            "contents": contents,
            "generationConfig": {
                "temperature": self.config.temperature,
                "maxOutputTokens": self.config.max_tokens,
            }
        });

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(anyhow::anyhow!("Gemini API error: {}", error));
        }

        let result: GeminiResponse = response.json().await?;
        Ok(result.candidates.first()
            .map(|c| c.content.parts.first().text)
            .unwrap_or_else(|| "No response".to_string()))
    }

    fn convert_messages(&self, messages: &[ChatMessage]) -> Vec<GeminiContent> {
        messages.iter().map(|msg| GeminiContent {
            role: msg.role.clone(),
            parts: vec![GeminiPart {
                text: msg.content.clone(),
            }],
        }).collect()
    }
}

#[derive(Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiCandidateContent,
}

#[derive(Deserialize)]
struct GeminiCandidateContent {
    parts: Vec<GeminiPart>,
}
```

## Phase 3: Frontend Setup

### 3.1 Project Structure

```
web/
├── package.json
├── tsconfig.json
├── vite.config.ts
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── vite-env.d.ts
│   ├── api/
│   │   ├── client.ts
│   │   └── types.ts
│   ├── components/
│   │   ├── Layout/
│   │   │   ├── MainLayout.tsx
│   │   │   ├── Header.tsx
│   │   │   └── Sider.tsx
│   │   ├── Dashboard/
│   │   │   ├── index.tsx
│   │   │   ├── StatsCard.tsx
│   │   │   ├── SystemHealth.tsx
│   │   │   └── RecentActivity.tsx
│   │   ├── Agents/
│   │   │   ├── index.tsx
│   │   │   ├── AgentList.tsx
│   │   │   ├── AgentEditor.tsx
│   │   │   └── AgentForm.tsx
│   │   ├── Skills/
│   │   │   ├── index.tsx
│   │   │   ├── SkillList.tsx
│   │   │   ├── SkillEditor.tsx
│   │   │   └── SkillForm.tsx
│   │   ├── Sessions/
│   │   │   ├── index.tsx
│   │   │   ├── SessionList.tsx
│   │   │   └── SessionDetail.tsx
│   │   ├── Files/
│   │   │   ├── index.tsx
│   │   │   └── FileEditor.tsx
│   │   ├── Marketplace/
│   │   │   ├── index.tsx
│   │   │   ├── SearchBar.tsx
│   │   │   └── SkillCard.tsx
│   │   └── common/
│   │       ├── PageHeader.tsx
│   │       ├── Loading.tsx
│   │       ├── ErrorBoundary.tsx
│   │       └── CodeEditor.tsx
│   ├── hooks/
│   │   ├── useAuth.ts
│   │   ├── useWebSocket.ts
│   │   └── useApi.ts
│   ├── stores/
│   │   ├── authStore.ts
│   │   ├── agentStore.ts
│   │   └── sessionStore.ts
│   └── styles/
│       └── global.css
└── dist/
    ├── index.html
    └── assets/
```

### 3.2 Package Dependencies

```json
{
  "name": "uhorse-web",
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "^18.2.0",
    "react-dom": "^18.2.0",
    "react-router-dom": "^6.20.0",
    "antd": "^5.12.0",
    "@ant-design/icons": "^5.2.0",
    "@ant-design/charts": "^2.0.0",
    "axios": "^1.6.0",
    "zustand": "^4.4.0",
    "react-query": "^3.39.0",
    "@monaco-editor/react": "^4.6.0",
    "dayjs": "^1.11.0"
  },
  "devDependencies": {
    "@types/react": "^18.2.0",
    "@types/react-dom": "^18.2.0",
    "typescript": "^5.3.0",
    "vite": "^5.0.0",
    "@vitejs/plugin-react": "^4.2.0"
  }
}
```

## Phase 4: Frontend Components

### 4.1 MainLayout

```tsx
import { Layout, Menu } from 'antd';
import { Routes, Route } from 'react-router-dom';
import Dashboard from './Dashboard';
import Agents from './Agents';
import Skills from './Skills';
import Sessions from './Sessions';
import Marketplace from './Marketplace';

const { Header, Sider, Content } = Layout;

export default function MainLayout() {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Sider collapsible collapsed={collapsed} onCollapse={setCollapsed}>
        <div className="logo" />
        <Menu theme="dark" mode="inline">
          <Menu.Item key="dashboard">
            <Link to="/"><DashboardOutlined /> Dashboard</Link>
          </Menu.Item>
          <Menu.Item key="agents">
            <Link to="/agents"><RobotOutlined /> Agents</Link>
          </Menu.Item>
          <Menu.Item key="skills">
            <Link to="/skills"><ToolOutlined /> Skills</Link>
          </Menu.Item>
          <Menu.Item key="sessions">
            <Link to="/sessions"><MessageOutlined /> Sessions</Link>
          </Menu.Item>
          <Menu.Item key="marketplace">
            <Link to="/marketplace"><ShopOutlined /> Marketplace</Link>
          </Menu.Item>
        </Menu>
      </Sider>
      <Layout>
        <Header style={{ padding: 0, background: '#fff' }}>
          {/* Header content */}
        </Header>
        <Content style={{ margin: '24px' }}>
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/agents" element={<Agents />} />
            <Route path="/skills" element={<Skills />} />
            <Route path="/sessions" element={<Sessions />} />
            <Route path="/marketplace" element={<Marketplace />} />
          </Routes>
        </Content>
      </Layout>
    </Layout>
  );
}
```

### 4.2 Dashboard

```tsx
import { Row, Col, Card, Statistic } from 'antd';
import { Line } from '@ant-design/charts';
import useWebSocket from '../hooks/useWebSocket';

export default function Dashboard() {
  const { stats, connected } = useWebSocket('/ws');

  return (
    <div>
      <Row gutter={16}>
        <Col span={6}>
          <Card>
            <Statistic title="Channels" value={stats?.channels || 0} />
          </Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic title="Agents" value={stats?.agents || 0} />
          </Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic title="Active Sessions" value={stats?.sessions || 0} />
          </Card>
        </Col>
        <Col span={6}>
          <Card>
            <Statistic title="Messages" value={stats?.messages || 0} />
          </Card>
        </Col>
      </Row>

      <Row gutter={16} style={{ marginTop: 16 }}>
        <Col span={16}>
          <Card title="Message Activity">
            <Line data={stats?.messageHistory || []} />
          </Card>
        </Col>
        <Col span={8}>
          <Card title="System Health">
            <SystemHealth />
          </Card>
        </Col>
      </Row>

      <Row style={{ marginTop: 16 }}>
        <Col span={24}>
          <Card title="Recent Activity">
            <RecentActivity />
          </Card>
        </Col>
      </Row>
    </div>
  );
}
```

### 4.3 Agent Editor

```tsx
import { Tabs, Form, Input, Button, Select } from 'antd';
import CodeEditor from '../common/CodeEditor';

export default function AgentEditor({ agentId }: { agentId?: string }) {
  const [form] = Form.useForm();
  const [soulContent, setSoulContent] = useState('');
  const [memoryContent, setMemoryContent] = useState('');

  return (
    <div>
      <Form form={form} layout="vertical">
        <Form.Item name="name" label="Name">
          <Input />
        </Form.Item>
        <Form.Item name="description" label="Description">
          <Input.TextArea />
        </Form.Item>
        <Form.Item name="model" label="Model">
          <Select>
            <Select.Option value="gpt-4">GPT-4</Select.Option>
            <Select.Option value="gpt-3.5-turbo">GPT-3.5 Turbo</Select.Option>
            <Select.Option value="claude-3">Claude 3</Select.Option>
          </Select>
        </Form.Item>
      </Form>

      <Tabs>
        <Tabs.TabPane tab="SOUL.md" key="soul">
          <CodeEditor
            value={soulContent}
            onChange={setSoulContent}
            language="markdown"
          />
        </Tabs.TabPane>
        <Tabs.TabPane tab="MEMORY.md" key="memory">
          <CodeEditor
            value={memoryContent}
            onChange={setMemoryContent}
            language="markdown"
          />
        </Tabs.TabPane>
      </Tabs>
    </div>
  );
}
```

## Phase 5: Skill Marketplace

### 5.1 Marketplace Backend

**File**: `crates/uhorse-tool/src/marketplace.rs`

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
pub struct MarketplaceSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub repository: String,
    pub downloads: u64,
    pub rating: f32,
    pub tags: Vec<String>,
    pub skill_manifest: String,
}

pub struct MarketplaceClient {
    registry_url: String,
    client: reqwest::Client,
    cache: HashMap<String, MarketplaceSkill>,
}

impl MarketplaceClient {
    pub fn new(registry_url: &str) -> Self {
        Self {
            registry_url: registry_url.to_string(),
            client: reqwest::Client::new(),
            cache: HashMap::new(),
        }
    }

    pub async fn search(&self, query: &str, tags: Option<&[String]>) -> Result<Vec<MarketplaceSkill>> {
        let mut url = format!("{}/search?q={}", self.registry_url, query);
        if let Some(t) = tags {
            url.push_str(&format!("&tags={}", t.join(",")));
        }

        let response = self.client.get(&url).send().await?;
        let skills: Vec<MarketplaceSkill> = response.json().await?;
        Ok(skills)
    }

    pub async fn get(&self, id: &str) -> Result<MarketplaceSkill> {
        if let Some(skill) = self.cache.get(id) {
            return Ok(skill.clone());
        }

        let url = format!("{}/skills/{}", self.registry_url, id);
        let response = self.client.get(&url).send().await?;
        let skill: MarketplaceSkill = response.json().await?;
        self.cache.insert(id.to_string(), skill.clone());
        Ok(skill)
    }

    pub async fn install(&self, id: &str, target_dir: &Path) -> Result<()> {
        let skill = self.get(id).await?;
        let skill_dir = target_dir.join(&skill.id);
        tokio::fs::create_dir_all(&skill_dir).await?;

        // Download skill files
        let manifest_path = skill_dir.join("SKILL.md");
        tokio::fs::write(&manifest_path, &skill.skill_manifest).await?;

        Ok(())
    }
}
```

## Phase 6: CI/CD

### 6.1 Frontend Build Workflow

**File**: `.github/workflows/web.yml`

```yaml
name: Build Web

on:
  push:
    branches: [main]
    paths:
      - 'web/**'
  pull_request:
    paths:
      - 'web/**'

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: web/package-lock.json

      - run: cd web && npm ci
      - run: cd web && npm run build

      - uses: actions/upload-artifact@v4
        with:
          name: web-dist
          path: web/dist
```

### 6.2 Integration in Release

Add to `.github/workflows/release.yml`:

```yaml
      - name: Download web assets
        uses: actions/download-artifact@v4
        with:
          name: web-dist
          path: web-dist

      - name: Package web assets
        run: |
          mkdir -p release-assets
          cp -r web-dist release-assets/web
```

## Implementation Timeline

| Week | Phase | Deliverables |
|------|-------|--------------|
| 1 | API Foundation | API types, routes, handlers |
| 2 | LLM Providers | Anthropic, Gemini clients |
| 3 | Frontend Setup | React project, layout, routing |
| 4 | Frontend Features | Dashboard, Agents, Skills |
| 5 | Marketplace | Search, install, publish |

## Success Criteria

- [ ] All API endpoints return correct responses
- [ ] Swagger/OpenAPI documentation generated
- [ ] OpenAI, Anthropic, Gemini LLM calls work
- [ ] Dashboard shows real-time stats
- [ ] Agent CRUD works with file editing
- [ ] Skill CRUD works with SKILL.md editor
- [ ] Marketplace search and install works
- [ ] WebSocket real-time updates work
- [ ] Integration tests pass
- [ ] Documentation updated
