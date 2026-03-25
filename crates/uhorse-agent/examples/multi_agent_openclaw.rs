//! # OpenClaw 风格多 Agent 示例
//!
//! 展示 uHorse Agent 框架的 OpenClaw 多 Agent 架构使用方式。
//! 每个 Agent 有独立的 workspace、SOUL.md、MEMORY.md。

use anyhow::Result;
use std::sync::Arc;
use uhorse_agent::agent_scope::{AgentManager, AgentScope, AgentScopeConfig};
use uhorse_agent::Agent;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("🦀 uHorse Agent Framework - OpenClaw Multi-Agent Example\n");
    println!("📁 Each Agent has its own workspace:\n");
    println!("   ~/.uhorse/workspace/         → Main Agent");
    println!("   ~/.uhorse/workspace-coder/   → Coder Agent");
    println!("   ~/.uhorse/workspace-writer/  → Writer Agent\n");

    // 检查 API Key
    let api_key = std::env::var("OPENAI_API_KEY");
    let has_api_key = api_key.is_ok() && !api_key.as_ref().unwrap().is_empty();

    if !has_api_key {
        println!("⚠️  Warning: OPENAI_API_KEY not set!");
        println!("   Set it with: export OPENAI_API_KEY=sk-xxx");
        println!("   Running in DEMO mode (will use mock responses)...\n");
    }

    // ============ 创建 Agent Manager ============
    println!("🔧 Creating Agent Manager...");
    let mut agent_manager = AgentManager::new(std::path::PathBuf::from("~/.uhorse"))?;

    // ============ 创建 Main Agent ============
    println!("🤖 Creating Main Agent...");
    let main_scope_config = AgentScopeConfig {
        agent_id: "main".to_string(),
        workspace_dir: std::path::PathBuf::from("~/.uhorse/workspace"),
        display_name: Some("Main Assistant".to_string()),
        is_default: true,
    };
    let main_scope = Arc::new(AgentScope::new(main_scope_config)?);
    main_scope.init_workspace().await?;
    agent_manager.register_scope(main_scope.clone())?;

    let _main_agent = Agent::builder()
        .agent_id("main")
        .name("Main Assistant")
        .description("Helpful AI assistant for general tasks")
        .workspace_dir("~/.uhorse/workspace")
        .system_prompt(
            "You are a helpful AI assistant for uHorse. \
            You can answer questions and help users with various tasks. \
            Please respond in Chinese.",
        )
        .set_default(true)
        .build()?;

    // ============ 创建 Coder Agent ============
    println!("💻 Creating Coder Agent...");
    let coder_scope_config = AgentScopeConfig {
        agent_id: "coder".to_string(),
        workspace_dir: std::path::PathBuf::from("~/.uhorse/workspace-coder"),
        display_name: Some("Code Expert".to_string()),
        is_default: false,
    };
    let coder_scope = Arc::new(AgentScope::new(coder_scope_config)?);
    coder_scope.init_workspace().await?;

    // 为 Coder Agent 设置专业的 SOUL.md
    tokio::fs::write(
        coder_scope.workspace_dir().join("SOUL.md"),
        "# Code Expert Persona\n\n\
         You are an expert programmer with deep knowledge in:\n\
         - Rust, TypeScript, Python\n\
         - System design and architecture\n\
         - Best practices and patterns\n\n\
         Your responses are:\n\
         - Technical and precise\n\
         - Include code examples when relevant\n\
         - Focus on correctness and performance",
    )
    .await?;

    agent_manager.register_scope(coder_scope.clone())?;

    let _coder_agent = Agent::builder()
        .agent_id("coder")
        .name("Code Expert")
        .description("Expert programmer agent for technical tasks")
        .workspace_dir("~/.uhorse/workspace-coder")
        .system_prompt(
            "You are an expert programmer. \
            Help users with code, architecture, and technical questions. \
            Please respond in Chinese.",
        )
        .build()?;

    // ============ 创建 Writer Agent ============
    println!("✍️  Creating Writer Agent...");
    let writer_scope_config = AgentScopeConfig {
        agent_id: "writer".to_string(),
        workspace_dir: std::path::PathBuf::from("~/.uhorse/workspace-writer"),
        display_name: Some("Creative Writer".to_string()),
        is_default: false,
    };
    let writer_scope = Arc::new(AgentScope::new(writer_scope_config)?);
    writer_scope.init_workspace().await?;

    // 为 Writer Agent 设置创意的 SOUL.md
    tokio::fs::write(
        writer_scope.workspace_dir().join("SOUL.md"),
        "# Creative Writer Persona\n\n\
         You are a creative writer with expertise in:\n\
         - Technical writing and documentation\n\
         - Blog posts and articles\n\
         - Creative storytelling\n\n\
         Your responses are:\n\
         - Engaging and well-structured\n\
         - Clear and concise\n\
         - Tailored to the target audience",
    )
    .await?;

    agent_manager.register_scope(writer_scope.clone())?;

    let _writer_agent = Agent::builder()
        .agent_id("writer")
        .name("Creative Writer")
        .description("Creative writer agent for content creation")
        .workspace_dir("~/.uhorse/workspace-writer")
        .system_prompt(
            "You are a creative writer. \
            Help users with writing, content creation, and documentation. \
            Please respond in Chinese.",
        )
        .build()?;

    println!("\n✅ All agents created successfully!\n");

    // ============ 演示 ============
    println!("═══════════════════════════════════════════════════════════");
    println!("  Demo: Each Agent has independent workspace and memory");
    println!("═══════════════════════════════════════════════════════════\n");

    // 显示每个 Agent 的 workspace 结构
    for scope in agent_manager.list_agents() {
        println!(
            "📂 {} Workspace:",
            scope
                .config()
                .display_name
                .as_ref()
                .unwrap_or(&scope.config().agent_id)
        );
        println!("   Path: {}", scope.workspace_dir().display());
        println!("   Files:");

        let soul_path = scope.workspace_dir().join("SOUL.md");
        if soul_path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&soul_path).await {
                let preview = content.lines().take(3).collect::<Vec<_>>().join("; ");
                println!("   - SOUL.md: {}", preview);
            }
        }

        println!();
    }

    // ============ 模拟会话 ============
    if has_api_key {
        println!("💬 Starting conversations with each agent...\n");

        // 使用 Main Agent
        println!("🤖 Main Agent:");
        println!("   User: 介绍一下 uHorse");
        println!("   Agent: （会读取 workspace/SOUL.md 中的性格设定）\n");

        // 使用 Coder Agent
        println!("💻 Coder Agent:");
        println!("   User: 如何在 Rust 中实现异步编程？");
        println!("   Agent: （会读取 workspace-coder/SOUL.md 中的技术专家性格）\n");

        // 使用 Writer Agent
        println!("✍️  Writer Agent:");
        println!("   User: 帮我写一篇技术博客的开头");
        println!("   Agent: （会读取 workspace-writer/SOUL.md 中的创意写作性格）\n");
    }

    // ============ 总结 ============
    println!("═══════════════════════════════════════════════════════════");
    println!("  OpenClaw 风格特性总结:");
    println!("═══════════════════════════════════════════════════════════");
    println!("✅ 每个 Agent 有独立 workspace");
    println!("✅ 独立的 SOUL.md（性格设定）");
    println!("✅ 独立的 MEMORY.md（长期记忆）");
    println!("✅ 独立的 memory/（每日日志）");
    println!("✅ 隔离的会话存储");
    println!("✅ 文件注入优先级: AGENTS.md > SOUL.md > MEMORY.md");
    println!("═══════════════════════════════════════════════════════════\n");

    println!("✅ 示例运行完成！");
    println!("\n💡 提示: 查看 ~/.uhorse/ 目录下的 workspace 结构");

    Ok(())
}
