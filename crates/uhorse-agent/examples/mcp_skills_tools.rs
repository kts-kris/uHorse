//! # MCP、Skills、Tools 完整示例
//!
//! 展示 uHorse Agent 框架的 MCP、Skills、Tools 集成使用。

use anyhow::Result;
use std::sync::Arc;
use uhorse_agent::{
    mcp::types::{McpContent, McpToolCall, McpToolResult},
    skills::{
        Skill, SkillConfig, SkillExecutor, SkillManifestParser, SkillPermission, SkillRegistry,
    },
    tools::{builtin_tools, Tool, ToolBuilder, ToolRegistry},
    AgentError, AgentResult,
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🦀 uHorse Agent Framework - MCP, Skills, Tools Example\n");

    // ============ 创建 Tool Registry ============
    println!("🔧 Creating Tool Registry...\n");

    let mut tool_registry = ToolRegistry::new();

    // 注册内置工具
    tool_registry.register(builtin_tools::get_current_time_tool());
    tool_registry.register(builtin_tools::calculator_tool());
    tool_registry.register(builtin_tools::text_search_tool());

    println!(
        "✅ Registered {} builtin tools:",
        tool_registry.list_names().len()
    );
    for name in tool_registry.list_names() {
        println!("   - {}", name);
    }
    println!();

    // ============ 创建自定义工具 ============
    println!("🛠️  Creating Custom Tools...\n");

    // 创建天气工具
    let weather_tool = ToolBuilder::new("get_weather")
        .description("Get weather information for a city")
        .add_param("city", "string", "City name", true)
        .build(|args| {
            let city = args["city"].as_str().unwrap_or("Unknown");

            // 模拟天气数据
            let weather_data = match city.to_lowercase().as_str() {
                "beijing" => "Sunny, 25°C",
                "shanghai" => "Cloudy, 22°C",
                "shenzhen" => "Rainy, 28°C",
                _ => "Partly Cloudy, 20°C",
            };

            Ok(McpToolResult {
                name: "get_weather".to_string(),
                content: vec![McpContent::Text {
                    text: format!("Weather in {}: {}", city, weather_data),
                }],
                is_error: false,
            })
        });

    tool_registry.register(weather_tool);
    println!("✅ Registered custom tool: get_weather\n");

    // ============ 创建 Skill（带 MCP Tools）============
    println!("📚 Creating Skill with MCP Tools...\n");

    let weather_skill_manifest = r#"
# Weather Skill

## Description
A skill for providing weather information for different cities.

## Version
1.0.0

## Tags
weather,information

## Tools
{
  "name": "get_forecast",
  "description": "Get weather forecast for the next few days",
  "inputSchema": {
    "type": "object",
    "properties": {
      "city": {"type": "string", "description": "City name"},
      "days": {"type": "number", "description": "Number of days"}
    },
    "required": ["city"]
  }
}
"#;

    let manifest = SkillManifestParser::parse_from_content(weather_skill_manifest)?;

    let weather_skill = Skill::new(
        manifest,
        SkillConfig {
            name: "weather".to_string(),
            enabled: true,
            permission: uhorse_agent::skills::SkillPermission::Normal,
            rate_limit: None,
        },
        Arc::new(WeatherSkillExecutor),
    );

    let mut skill_registry = SkillRegistry::new();
    skill_registry.register(weather_skill.clone());

    tool_registry.register_skill(weather_skill);

    println!("✅ Created Weather Skill with tool: get_forecast\n");

    // ============ 演示工具调用 ============
    println!("═══════════════════════════════════════════════════════════");
    println!("  Demo: Tool Execution");
    println!("═══════════════════════════════════════════════════════════\n");

    // 测试1: 内置工具 - 计算器
    println!("🧮 Testing Calculator Tool:");
    let calc_call = McpToolCall {
        name: "calculator".to_string(),
        arguments: serde_json::json!({"expression": "10 + 5"}),
    };

    let result = tool_registry.call(&calc_call).await?;
    println!("   Expression: 10 + 5");
    if let Some(McpContent::Text { text }) = result.content.first() {
        println!("   Result: {}\n", text);
    }

    // 测试2: 自定义工具 - 天气
    println!("🌤️  Testing Weather Tool:");
    let weather_call = McpToolCall {
        name: "get_weather".to_string(),
        arguments: serde_json::json!({"city": "Beijing"}),
    };

    let result = tool_registry.call(&weather_call).await?;
    println!("   City: Beijing");
    if let Some(McpContent::Text { text }) = result.content.first() {
        println!("   Result: {}\n", text);
    }

    // 测试3: Skill 工具 - 天气预报
    println!("🌦️  Testing Skill Tool (get_forecast):");
    let forecast_call = McpToolCall {
        name: "get_forecast".to_string(),
        arguments: serde_json::json!({"city": "Shenzhen", "days": 3}),
    };

    println!("   Calling forecast tool...");
    let result = tool_registry.call(&forecast_call).await?;
    println!("   City: Shenzhen, Days: 3");
    let result_text = result.content.first().and_then(|c| {
        if let McpContent::Text { text } = c {
            Some(text)
        } else {
            None
        }
    });
    println!(
        "   Result: {}\n",
        result_text.map(|s| s.as_str()).unwrap_or("No result")
    );
    println!("   ✅ Forecast test completed\n");

    // ============ 展示 MCP 协议支持 ============
    println!("═══════════════════════════════════════════════════════════");
    println!("  MCP Protocol Support");
    println!("═══════════════════════════════════════════════════════════\n");

    println!("✅ MCP Core Concepts:");
    println!("   • Tools: Functions controlled by the model");
    println!("   • Resources: Data sources accessed via URI");
    println!("   • Prompts: Reusable prompt templates");
    println!("   • Sessions: Connection and state management\n");

    println!("✅ MCP Integration:");
    println!("   • JSON-RPC 2.0 protocol");
    println!("   • Stdio/HTTP/WebSocket transport");
    println!("   • Language-agnostic implementation");
    println!("   • Cross-platform support\n");

    println!("✅ Available Tools (via MCP):");
    for tool in tool_registry.get_all_mcp_tools() {
        println!("   - {}: {}", tool.name, tool.description);
    }
    println!();

    // ============ 总结 ============
    println!("═══════════════════════════════════════════════════════════");
    println!("  Features Summary");
    println!("═══════════════════════════════════════════════════════════\n");

    println!("✅ MCP (Model Context Protocol)");
    println!("   • Tools - 外部函数调用");
    println!("   • Resources - 数据源访问");
    println!("   • Prompts - 提示词模板\n");

    println!("✅ Skills (OpenClaw 风格)");
    println!("   • SKILL.md 解析");
    println!("   • 独立的技能配置");
    println!("   • 技能权限管理");
    println!("   • MCP Tools 集成\n");

    println!("✅ Tools System");
    println!("   • Tool Registry");
    println!("   • JSON Schema 参数验证");
    println!("   • Builder 模式");
    println!("   • 内置工具库\n");

    println!("✅ 示例运行完成！");
    println!("\n💡 提示: uHorse 现在支持完整的 MCP 协议、Skills 和 Tools 系统");

    Ok(())
}

/// 天气技能执行器
struct WeatherSkillExecutor;

#[async_trait::async_trait]
impl SkillExecutor for WeatherSkillExecutor {
    async fn execute_tool(&self, call: &McpToolCall) -> AgentResult<McpToolResult> {
        match call.name.as_str() {
            "get_forecast" => {
                let city = call.arguments["city"].as_str().unwrap_or("Unknown");
                let days = call.arguments["days"].as_u64().unwrap_or(1);

                let forecast = format!("{}-day forecast for {}: Sunny, 20-25°C", days, city);

                Ok(McpToolResult {
                    name: call.name.clone(),
                    content: vec![McpContent::Text { text: forecast }],
                    is_error: false,
                })
            }
            _ => Err(AgentError::Skill(format!("Unknown tool: {}", call.name))),
        }
    }

    fn manifest(&self) -> &uhorse_agent::skills::SkillManifest {
        // 返回一个静态的 manifest
        static MANIFEST: std::sync::OnceLock<uhorse_agent::skills::SkillManifest> =
            std::sync::OnceLock::new();

        MANIFEST.get_or_init(|| {
            let content = r#"
# Weather Skill

## Description
A skill for providing weather information.

## Version
1.0.0
"#;

            SkillManifestParser::parse_from_content(content).unwrap()
        })
    }

    fn config(&self) -> &uhorse_agent::skills::SkillConfig {
        static CONFIG: std::sync::OnceLock<uhorse_agent::skills::SkillConfig> =
            std::sync::OnceLock::new();

        CONFIG.get_or_init(|| uhorse_agent::skills::SkillConfig {
            name: "weather".to_string(),
            enabled: true,
            permission: uhorse_agent::skills::SkillPermission::Normal,
            rate_limit: None,
        })
    }
}
