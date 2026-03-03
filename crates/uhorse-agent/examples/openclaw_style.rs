//! # OpenClaw 风格示例
//!
//! 展示 uHorse Agent 框架的 OpenClaw 四层架构使用方式。

use anyhow::Result;
use std::sync::Arc;
use uhorse_agent::memory::FileMemory;
use uhorse_agent::{Agent, AgentBuilder, Gateway, GatewayConfig};
use uhorse_llm::{LLMClient, OpenAIClient};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🦀 uHorse Agent Framework - OpenClaw Style Example\n");

    // 检查 API Key
    let api_key = std::env::var("OPENAI_API_KEY");
    let has_api_key = api_key.is_ok() && !api_key.as_ref().unwrap().is_empty();

    if !has_api_key {
        println!("⚠️  Warning: OPENAI_API_KEY not set!");
        println!("   Set it with: export OPENAI_API_KEY=sk-xxx");
        println!("   Running in DEMO mode (will use mock responses)...\n");
    }

    // 1. 创建 Memory（记忆系统）
    println!("📝 Initializing Memory (OpenClaw style: MEMORY.md, SOUL.md, USER.md)...");
    let workspace_dir = std::path::PathBuf::from("~/.uhorse/workspace-example");
    let workspace_str = workspace_dir.to_string_lossy().to_string();
    let expanded_dir = shellexpand::tilde(&workspace_str);
    let workspace_path = std::path::PathBuf::from(expanded_dir.as_ref());
    let memory = Arc::new(FileMemory::new(workspace_path.clone()));
    memory.init_workspace().await?;

    // 2. 创建 LLM 客户端
    println!("🤖 Initializing LLM client...");

    // 3. 创建 Gateway（控制平面）
    println!("🎯 Initializing Gateway (Control Plane)...");

    if has_api_key {
        // 使用真实的 OpenAI 客户端
        use uhorse_llm::OpenAIClient;
        let llm_client = Arc::new(OpenAIClient::from_env(&api_key.unwrap(), "gpt-4o")?);

        let config = GatewayConfig {
            workspace_dir: workspace_path,
            max_sessions: 100,
            session_timeout: 3600,
            enable_memory_persistence: true,
        };

        let gateway = Gateway::new(config, llm_client, memory.clone()).await?;

        // 4. 创建并注册 Agent
        println!("🤖 Creating Agents...");
        let assistant_agent = Agent::builder()
            .name("assistant")
            .description("Helpful AI assistant")
            .system_prompt(
                "You are a helpful AI assistant for uHorse. \
                You can answer questions and help users with various tasks. \
                Please respond in Chinese.",
            )
            .build()?;

        gateway.register_agent(assistant_agent).await?;

        // 5. 运行会话示例
        run_with_gateway(gateway, "user_12345".to_string()).await?;
    } else {
        // 使用模拟客户端
        let gateway = create_demo_gateway(workspace_path, memory).await?;

        // 4. 创建并注册 Agent
        println!("🤖 Creating Agents...");
        let assistant_agent = Agent::builder()
            .name("assistant")
            .description("Helpful AI assistant")
            .system_prompt(
                "You are a helpful AI assistant for uHorse. \
                You can answer questions and help users with various tasks. \
                Please respond in Chinese.",
            )
            .build()?;

        gateway.register_agent(assistant_agent).await?;

        // 5. 运行会话示例
        run_with_gateway(gateway, "user_12345".to_string()).await?;
    }

    Ok(())
}

async fn run_with_gateway<C>(gateway: Gateway<C>, user_id: String) -> Result<()>
where
    C: LLMClient + Send + Sync + Sized + 'static,
{
    println!("\n💬 Starting conversation...\n");

    // 创建或获取会话
    let (session_id, is_new) = gateway.get_or_create_session(&user_id).await?;
    if is_new {
        println!("✓ New session created: {}", session_id);
    } else {
        println!("✓ Existing session loaded: {}", session_id);
    }

    // 发送消息
    let user_message = "你好！请介绍一下 uHorse 是什么？";
    println!("👤 User: {}", user_message);

    let response = gateway.handle_message(&session_id, user_message).await?;
    println!("🤖 Assistant: {}", response.content);

    // 再次交互
    let user_message2 = "它支持哪些消息通道？";
    println!("\n👤 User: {}", user_message2);

    let response2 = gateway.handle_message(&session_id, user_message2).await?;
    println!("🤖 Assistant: {}", response2.content);

    // 6. 展示记忆系统
    println!("\n📂 Memory System (OpenClaw style):");
    println!("   Workspace: ~/.uhorse/workspace-example");
    println!("   - MEMORY.md: 长期记忆");
    println!("   - SOUL.md: 性格设定");
    println!("   - USER.md: 用户偏好");
    println!("   - sessions/{}/: 会话历史", session_id);

    println!("\n✅ 示例运行完成！");

    Ok(())
}

/// 创建演示模式的 Gateway
async fn create_demo_gateway(
    workspace_dir: std::path::PathBuf,
    memory: Arc<FileMemory>,
) -> Result<Gateway<DemoLLMClient>> {
    let config = GatewayConfig {
        workspace_dir,
        max_sessions: 100,
        session_timeout: 3600,
        enable_memory_persistence: true,
    };

    Gateway::new(config, Arc::new(DemoLLMClient), memory)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create gateway: {}", e))
}

/// 模拟 LLM 客户端（用于演示）
struct DemoLLMClient;

#[async_trait::async_trait]
impl LLMClient for DemoLLMClient {
    async fn chat_completion(
        &self,
        messages: Vec<uhorse_llm::ChatMessage>,
    ) -> anyhow::Result<String> {
        let last_msg = messages.last().unwrap();
        let content = &last_msg.content;

        if content.contains("uHorse 是什么") {
            Ok("uHorse 是一个基于 Rust 构建的多渠道 AI 网关框架，采用 OpenClaw 四层架构设计：Gateway（控制平面）、Agent（智能体）、Skills（技能系统）、Memory（记忆系统）。\n\n它支持 Telegram、Discord、Slack 等多个消息通道，可以让你从任何地方与 AI 助手进行交互。".to_string())
        } else if content.contains("支持哪些消息通道") {
            Ok("uHorse 目前支持以下消息通道：\n\n1. **Telegram** - 已完全支持\n2. **Discord** - 开发中\n3. **Slack** - 开发中\n4. **WhatsApp** - 规划中\n\n你可以通过配置文件启用或禁用特定通道。".to_string())
        } else {
            Ok(format!("（演示模式）收到你的消息：{}", content))
        }
    }
}
