# uHorse Agent Framework 设计文档

## 基于 OpenClaw 四层架构

本文档描述了 uHorse Agent 框架如何参考 OpenClaw 的设计理念，构建一套完整的 Agent 体系。

---

## OpenClaw 核心架构分析

### 四层架构

```
┌─────────────────────────────────────────────────────────────────┐
│                        Gateway (控制平面)                        │
│  - 会话管理  - 消息路由  - 多通道统一接口  - WebSocket 协议     │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                         Agent (智能体)                          │
│  - LLM 调用  - 工具使用  - 意图识别  - 多 Agent 协作           │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                        Skills (技能系统)                        │
│  - SKILL.md 描述  - .py/.ts 执行  - 参数验证  - 权限控制     │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                        Memory (记忆系统)                        │
│  - MEMORY.md  - SOUL.md  - USER.md  - 文件系统存储            │
└─────────────────────────────────────────────────────────────────┘
```

### 关键设计原则

1. **Gateway 作为单一真相来源**
   - 所有会话和路由逻辑集中管理
   - 事件驱动架构
   - 统一的消息协议

2. **Agent 作为智能执行单元**
   - 每个Agent有明确的职责
   - 支持Agent间协作和移交
   - LLM + 工具调用

3. **Skills 作为可扩展能力**
   - SKILL.md 描述（AI可读）
   - 独立的执行逻辑
   - 热插拔支持

4. **Memory 作为持久化层**
   - 记忆即文件
   - 多层次记忆（全局/会话/临时）
   - 人类可读可编辑

---

## uHorse vs OpenClaw 对比

| 维度 | OpenClaw (TS) | uHorse (Rust) |
|------|---------------|---------------|
| **Gateway** | ✅ 事件驱动 + WebSocket | ✅ 异步事件驱动 |
| **Agent** | ✅ LLM + 函数调用 | ✅ LLM + 技能调用 |
| **Skills** | SKILL.md + .py/.ts | SKILL.md + .rs/WASM |
| **Memory** | 文件系统 | 文件系统 + SQLite |
| **并发** | asyncio | Tokio |
| **类型安全** | 运行时 | 编译时 |
| **性能** | 中等 | 高 |
| **安全性** | 中等 | 高 |

---

## uHorse 架构设计

### 1. Gateway - 控制平面

```rust
pub struct Gateway<C> where C: LLMClient {
    config: GatewayConfig,
    llm_client: Arc<C>,
    memory: Arc<dyn MemoryStore>,
    router: Arc<Router>,
    agents: Arc<RwLock<HashMap<String, Agent>>>,
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    event_sender: UnboundedSender<GatewayEvent>,
}
```

**职责：**
- 会话管理（创建、获取、更新、删除）
- 消息路由到正确的 Agent
- 事件发布和订阅
- 多通道统一接口

### 2. Agent - 智能体

```rust
pub struct Agent {
    config: AgentConfig,
}

pub struct AgentConfig {
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    pub model: Option<String>,
    pub skills: SkillRegistry,
}
```

**职责：**
- LLM 调用和响应处理
- 技能/工具调用
- 意图识别
- 多 Agent 协作

### 3. Skills - 技能系统

```rust
pub struct Skill {
    pub manifest: SkillManifest,  // SKILL.md 解析结果
    pub config: SkillConfig,      // skill.toml
    executor: Arc<dyn SkillExecutor>,
}

pub trait SkillExecutor {
    async fn execute(&self, input: &str) -> AgentResult<String>;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
}
```

**技能结构：**
```
workspace/skills/my-skill/
├── SKILL.md          # 技能描述（AI 阅读）
├── mod.rs            # Rust 执行逻辑
├── skill.toml        # 技能配置
└── examples/         # 使用示例
```

### 4. Memory - 记忆系统

```rust
pub trait MemoryStore {
    async fn store_message(&self, session_id, user_msg, assistant_msg);
    async fn get_context(&self, session_id) -> AgentResult<String>;
    async fn store_kv(&self, session_id, key, value);
    async fn get_kv(&self, session_id, key) -> AgentResult<Option<String>>;
}
```

**目录结构：**
```
~/.uhorse/workspace/
├── MEMORY.md         # 长期记忆
├── SOUL.md           # 性格设定
├── USER.md           # 用户偏好
├── HEARTBEAT.md      # 状态记录
├── TODO.md           # 任务列表
└── sessions/
    └── {session_id}/
        ├── history.md # 会话历史
        └── kv.json    # 键值存储
```

---

## 使用示例

```rust
// 1. 初始化 Gateway
let memory = Arc::new(FileMemory::new(workspace_dir));
let gateway = Gateway::new(config, llm_client, memory).await?;

// 2. 注册 Agent
let agent = Agent::builder()
    .name("assistant")
    .system_prompt("You are a helpful assistant")
    .build()?;
gateway.register_agent(agent).await?;

// 3. 处理消息
let response = gateway.handle_message(&session_id, "Hello!").await?;
```

---

## 设计优势

1. **类型安全**：Rust 编译时检查
2. **高性能**：零成本抽象 + 异步
3. **可扩展**：基于 Trait 的插件系统
4. **OpenClaw 兼容**：相同的架构理念和文件结构

---

## 下一步

- [ ] Skills 动态加载
- [ ] WASM 技能支持
- [ ] 多 Agent 协作
- [ ] 技能市场（ClawHub 风格）
- [ ] Web UI 控制面板
