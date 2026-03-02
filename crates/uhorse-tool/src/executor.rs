//! # 工具执行器
//!
//! 执行工具调用的核心逻辑。

use uhorse_core::{ToolExecutor, ExecutionContext, Result, ToolId, PermissionLevel};

/// 示例工具：天气查询
#[derive(Debug)]
pub struct WeatherTool {
    id: ToolId,
}

impl WeatherTool {
    pub fn new() -> Self {
        Self {
            id: ToolId("weather".to_string()),
        }
    }
}

impl Default for WeatherTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ToolExecutor for WeatherTool {
    fn id(&self) -> &ToolId {
        &self.id
    }

    fn name(&self) -> &str {
        "weather"
    }

    fn description(&self) -> &str {
        "Get weather information for a city"
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        // 使用静态引用
        &*Box::leak(Box::new(serde_json::json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "City name"
                }
            },
            "required": ["city"]
        })))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Public
    }

    async fn execute(&self, params: serde_json::Value, _context: &ExecutionContext) -> Result<serde_json::Value> {
        let city = params["city"].as_str().unwrap_or("Unknown");
        Ok(serde_json::json!({
            "city": city,
            "temperature": 25,
            "condition": "Sunny",
        }))
    }
}

// 导出内置工具
pub use crate::tools::{
    CalculatorTool, HttpTool, SearchTool, DatetimeTool, TextTool,
};

