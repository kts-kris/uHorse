# uHorse Agent Framework

基于 **OpenClaw 四层架构**（Gateway-Agent-Skills-Memory）设计的多智能体系统。

## 🎯 核心架构

```
┌─────────────────────────────────────────────────────────────────┐
│                        Gateway (控制平面)                        │
│  - 会话管理  - 消息路由  - 多通道统一接口  - 事件驱动           │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                         Agent (智能体)                          │
│  - 独立 Workspace  - LLM 调用  - 技能使用  - 意图识别           │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                        Skills (技能系统)                        │
│  - SKILL.md 描述  - Rust/WASM 执行  - 参数验证  - 权限控制     │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                        Memory (记忆系统)                        │
│  - MEMORY.md  - SOUL.md  - USER.md  - 文件系统 + SQLite       │
└─────────────────────────────────────────────────────────────────┘
```

## ✨ OpenClaw 风格特性

### 🧩 多 Agent 架构

每个 Agent 拥有**独立的 workspace**：

```
~/.uhorse/
├── workspace/              # Main Agent
│   ├── SOUL.md             # 独立性格
│   ├── MEMORY.md           # 独立记忆
│   └── memory/             # 每日日志
│
├── workspace-coder/        # Coder Agent
│   ├── SOUL.md             # 技术专家性格
│   └── MEMORY.md           # 技术知识库
│
├── workspace-writer/       # Writer Agent
│   ├── SOUL.md             # 创意作家性格
│   └── MEMORY.md           # 写作知识库
│
└── agents/                 # Agent 状态存储
    ├── main/sessions/
    ├── coder/sessions/
    └── writer/sessions/
```

### 📋 Bindings 路由系统

根据 Session Key 自动路由到合适的 Agent：

```rust
use uhorse_agent::{BindingsConfig, BindingsRouter, BindingBuilder, SessionKey};

let mut bindings = BindingsConfig::new().default_agent("main");

// Telegram 所有消息 -> Main Agent
bindings.add_binding(
    BindingBuilder::new("main")
        .channel("telegram")
        .build()
);

// Slack 工作区 T123 -> Coder Agent
bindings.add_binding(
    BindingBuilder::new("coder")
        .channel("slack")
        .team_id("T123")
        .priority(10)
        .build()
);

let router = BindingsRouter::new(bindings, agents);

// 路由消息
let session_key = SessionKey::with_team("slack", "user123", "T123");
let agent_id = router.route(&session_key)?; // → "coder"
```

### 🔑 Session Key 格式

```
{channel_type}:{user_id}[:{team_id}]
```

示例：
- `telegram:user123` - Telegram 用户会话
- `slack:user456:T123` - Slack 工作区会话
- `discord:user789` - Discord 用户会话

### 📄 文件注入优先级

| 文件 | 注入时机 | 谁可修改 |
|------|----------|----------|
| AGENTS.md | 每个会话 | 仅人类 |
| SOUL.md | 每个会话 | Agent |
| MEMORY.md | 仅主会话 | Agent |
| memory/YYYY-MM-DD.md | 每个会话 | Agent |

## 📋 OpenClaw vs uHorse

| 特性 | OpenClaw (TS) | uHorse (Rust) |
|------|---------------|---------------|
| 多 Agent Workspace | ✅ | ✅ |
| Bindings 路由 | ✅ | ✅ |
| Session Key | ✅ | ✅ |
| 文件注入优先级 | ✅ | ✅ |
| Skills | SKILL.md + .py/.ts | SKILL.md + .rs/WASM |
| Memory | 文件系统 | 文件系统 + SQLite |
| 并发 | asyncio | ✅ Tokio |
| 类型安全 | 运行时 | ✅ 编译时 |
| 性能 | 中等 | ✅ 高 |
| 安全性 | 中等 | ✅ 高 |

## 🚀 快速开始

### 多 Agent 示例

```rust
use uhorse_agent::{
    Agent, AgentBuilder,
    AgentScope, AgentScopeConfig, AgentManager,
    BindingsConfig, BindingsRouter, BindingBuilder,
    SessionKey,
};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 创建 Agent Manager
    let mut manager = AgentManager::new("~/.uhorse".into())?;

    // 2. 创建 Main Agent
    let main_scope = Arc::new(AgentScope::new(AgentScopeConfig {
        agent_id: "main".to_string(),
        workspace_dir: "~/.uhorse/workspace".into(),
        is_default: true,
    })?);
    main_scope.init_workspace().await?;
    manager.register_scope(main_scope);

    let main_agent = Agent::builder()
        .agent_id("main")
        .name("Main Assistant")
        .workspace_dir("~/.uhorse/workspace")
        .build()?;

    // 3. 创建 Coder Agent
    let coder_scope = Arc::new(AgentScope::new(AgentScopeConfig {
        agent_id: "coder".to_string(),
        workspace_dir: "~/.uhorse/workspace-coder".into(),
        is_default: false,
    })?);
    coder_scope.init_workspace().await?;
    manager.register_scope(coder_scope);

    let coder_agent = Agent::builder()
        .agent_id("coder")
        .name("Code Expert")
        .workspace_dir("~/.uhorse/workspace-coder")
        .build()?;

    // 4. 配置路由
    let mut bindings = BindingsConfig::new().default_agent("main");
    bindings.add_binding(
        BindingBuilder::new("coder")
            .channel("slack")
            .team_id("T123")
            .build()
    );

    let router = BindingsRouter::new(bindings, vec![
        "main".to_string(),
        "coder".to_string(),
    ]);

    // 5. 路由消息
    let session_key = SessionKey::with_team("slack", "user123", "T123");
    let agent_id = router.route(&session_key)?; // → "coder"

    Ok(())
}
```

### 运行示例

```bash
# 多 Agent 示例
cargo run --example multi_agent_openclaw

# Bindings 路由示例
cargo run --example bindings_routing

# 简单对话示例
cargo run --example openclaw_style
```

## 📚 更多文档

- [架构设计文档](ARCHITECTURE.md)
- [OpenClaw 官方文档](https://docs.openclaw.ai/)
- [OpenClaw GitHub](https://github.com/openclaw/openclaw)

## 📄 许可证

MIT OR Apache-2.0
