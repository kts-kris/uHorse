//! # OpenClaw 风格 Bindings 路由示例
//!
//! 展示 uHorse Agent 框架的 OpenClaw Bindings 路由系统使用方式。

use uhorse_agent::{
    Agent, AgentBuilder,
    AgentScope, AgentScopeConfig, AgentManager,
    BindingsConfig, BindingsRouter, BindingBuilder,
    SessionKey, ChannelType,
};
use std::sync::Arc;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🦀 uHorse Agent Framework - OpenClaw Bindings Routing Example\n");

    // ============ 创建 Agent Manager ============
    println!("🔧 Creating Agent Manager...");
    let mut agent_manager = AgentManager::new(std::path::PathBuf::from("~/.uhorse"))?;

    // ============ 创建多个 Agents ============
    println!("🤖 Creating Agents with independent workspaces...\n");

    // Main Agent
    let main_scope = Arc::new(AgentScope::new(AgentScopeConfig {
        agent_id: "main".to_string(),
        workspace_dir: std::path::PathBuf::from("~/.uhorse/workspace-routing-main"),
        display_name: Some("Main Assistant".to_string()),
        is_default: true,
    })?);
    main_scope.init_workspace().await?;
    agent_manager.register_scope(main_scope);

    let _main_agent = Agent::builder()
        .agent_id("main")
        .name("Main Assistant")
        .description("General purpose assistant")
        .workspace_dir("~/.uhorse/workspace-routing-main")
        .system_prompt("You are a helpful assistant.")
        .build()?;

    // Coder Agent
    let coder_scope = Arc::new(AgentScope::new(AgentScopeConfig {
        agent_id: "coder".to_string(),
        workspace_dir: std::path::PathBuf::from("~/.uhorse/workspace-routing-coder"),
        display_name: Some("Code Expert".to_string()),
        is_default: false,
    })?);
    coder_scope.init_workspace().await?;
    agent_manager.register_scope(coder_scope);

    let _coder_agent = Agent::builder()
        .agent_id("coder")
        .name("Code Expert")
        .description("Expert programmer")
        .workspace_dir("~/.uhorse/workspace-routing-coder")
        .system_prompt("You are an expert programmer.")
        .build()?;

    // Writer Agent
    let writer_scope = Arc::new(AgentScope::new(AgentScopeConfig {
        agent_id: "writer".to_string(),
        workspace_dir: std::path::PathBuf::from("~/.uhorse/workspace-routing-writer"),
        display_name: Some("Creative Writer".to_string()),
        is_default: false,
    })?);
    writer_scope.init_workspace().await?;
    agent_manager.register_scope(writer_scope);

    let _writer_agent = Agent::builder()
        .agent_id("writer")
        .name("Creative Writer")
        .description("Content creator")
        .workspace_dir("~/.uhorse/workspace-routing-writer")
        .system_prompt("You are a creative writer.")
        .build()?;

    // ============ 配置 Bindings 路由 ============
    println!("📋 Configuring Bindings Routing...\n");

    let mut bindings_config = BindingsConfig::new()
        .default_agent("main");

    // Telegram 所有消息 -> Main Agent
    bindings_config.add_binding(
        BindingBuilder::new("main")
            .channel("telegram")
            .build()
    );

    // Slack 工作区 T123 (开发团队) -> Coder Agent
    bindings_config.add_binding(
        BindingBuilder::new("coder")
            .channel("slack")
            .team_id("T123")
            .priority(10)
            .build()
    );

    // Slack 工作区 T456 (内容团队) -> Writer Agent
    bindings_config.add_binding(
        BindingBuilder::new("writer")
            .channel("slack")
            .team_id("T456")
            .priority(10)
            .build()
    );

    // Discord 所有消息 -> Main Agent
    bindings_config.add_binding(
        BindingBuilder::new("main")
            .channel("discord")
            .build()
    );

    // 创建路由器
    let available_agents = vec![
        "main".to_string(),
        "coder".to_string(),
        "writer".to_string(),
    ];

    let router = BindingsRouter::new(bindings_config, available_agents);

    println!("✅ Bindings configured successfully!\n");

    // ============ 演示路由 ============
    println!("═══════════════════════════════════════════════════════════");
    println!("  Demo: Routing Messages to Different Agents");
    println!("═══════════════════════════════════════════════════════════\n");

    // 测试用例: (session_key, expected_agent)
    let test_cases = vec![
        // Telegram -> Main
        (SessionKey::new("telegram", "user123"), "main"),

        // Slack T123 -> Coder
        (SessionKey::with_team("slack", "dev_user", "T123"), "coder"),

        // Slack T456 -> Writer
        (SessionKey::with_team("slack", "content_user", "T456"), "writer"),

        // Slack 其他 -> Main (默认)
        (SessionKey::new("slack", "other_user"), "main"),

        // Discord -> Main
        (SessionKey::new("discord", "user789"), "main"),

        // 未知 channel -> Main (默认)
        (SessionKey::new("whatsapp", "user999"), "main"),
    ];

    println!("📬 Routing Test Results:\n");

    for (session_key, expected) in test_cases {
        let routed = router.route(&session_key).unwrap();
        let status = if routed == expected { "✅" } else { "❌" };

        println!("{} {}", status, session_key.as_str());
        println!("   → Agent: {} (expected: {})\n", routed, expected);
    }

    // ============ 展示 Session Key 生成 ============
    println!("═══════════════════════════════════════════════════════════");
    println!("  Session Key Format");
    println!("═══════════════════════════════════════════════════════════\n");

    println!("Format: {{channel_type}}:{{user_id}}[:{{team_id}}]\n");
    println!("Examples:");
    println!("  - telegram:user123");
    println!("  - slack:dev_user:T123");
    println!("  - discord:user789");
    println!("  - whatsapp:user999\n");

    // ============ 展示 Bindings 配置优先级 ============
    println!("═══════════════════════════════════════════════════════════");
    println!("  Routing Priority");
    println!("═══════════════════════════════════════════════════════════\n");

    println!("1. 精确匹配: channel + teamId");
    println!("   Example: slack:user:T123 → Coder Agent (priority 10)\n");

    println!("2. 通道匹配: 仅 channel");
    println!("   Example: telegram:user123 → Main Agent\n");

    println!("3. 默认: 使用 default_agent");
    println!("   Example: whatsapp:user999 → Main Agent\n");

    // ============ 总结 ============
    println!("═══════════════════════════════════════════════════════════");
    println!("  OpenClaw 风格 Bindings 特性总结");
    println!("═══════════════════════════════════════════════════════════\n");

    println!("✅ 灵活的路由规则配置");
    println!("✅ 支持多通道 (Telegram, Slack, Discord, etc.)");
    println!("✅ 支持团队/工作区隔离 (teamId)");
    println!("✅ 优先级系统");
    println!("✅ 用户白名单");
    println!("✅ 默认 Agent fallback");
    println!("✅ Session Key 唯一标识会话");
    println!("═══════════════════════════════════════════════════════════\n");

    println!("✅ 示例运行完成！");
    println!("\n💡 提示: 查看 ~/.uhorse/workspace-routing-* 目录下的独立 workspaces");

    Ok(())
}
