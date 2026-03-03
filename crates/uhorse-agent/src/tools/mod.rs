//! # Tools - 工具调用系统
//!
//! MCP 风格的工具调用系统，支持：
//! - 函数定义
//! - JSON Schema 参数验证
//! - 工具注册和调用
//! - 结果格式化

use crate::error::{AgentError, AgentResult};
use crate::mcp::types::{McpTool, McpToolCall, McpToolResult, McpContent};
use crate::skills::{Skill, SkillRegistry};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// 工具函数签名
pub type ToolFn = Arc<dyn Fn(Value) -> AgentResult<McpToolResult> + Send + Sync>;

/// 工具定义
#[derive(Clone)]
pub struct Tool {
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 输入 JSON Schema
    pub input_schema: Value,
    /// 执行函数
    executor: ToolFn,
}

impl Tool {
    /// 创建新工具
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        executor: ToolFn,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            executor,
        }
    }

    /// 转换为 MCP Tool
    pub fn to_mcp_tool(&self) -> McpTool {
        McpTool {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
        }
    }

    /// 执行工具
    pub async fn execute(&self, arguments: Value) -> AgentResult<McpToolResult> {
        // 验证参数
        self.validate_arguments(&arguments)?;

        // 执行
        (self.executor)(arguments)
    }

    /// 验证参数
    fn validate_arguments(&self, arguments: &Value) -> AgentResult<()> {
        // 简单验证：检查是否是对象
        if !arguments.is_object() {
            return Err(AgentError::InvalidConfig(
                "Tool arguments must be a JSON object".to_string(),
            ));
        }

        // TODO: 使用 jsonschema crate 进行完整验证
        Ok(())
    }
}

/// 工具注册表
#[derive(Clone)]
pub struct ToolRegistry {
    /// 工具映射
    tools: HashMap<String, Tool>,
    /// 技能注册表（用于查找技能中的工具）
    skills: SkillRegistry,
}

impl ToolRegistry {
    /// 创建新的注册表
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            skills: SkillRegistry::new(),
        }
    }

    /// 注册工具
    pub fn register(&mut self, tool: Tool) {
        let name = tool.name.clone();
        self.tools.insert(name, tool);
    }

    /// 注册技能（包含其工具）
    pub fn register_skill(&mut self, skill: Skill) {
        // 注册技能到技能注册表
        self.skills.register(skill);
    }

    /// 获取工具
    pub fn get(&self, name: &str) -> Option<Tool> {
        // 只查找独立注册的工具
        // 技能工具由 call 方法特殊处理
        if let Some(tool) = self.tools.get(name) {
            return Some(Tool {
                name: tool.name.clone(),
                description: tool.description.clone(),
                input_schema: tool.input_schema.clone(),
                executor: tool.executor.clone(),
            });
        }

        None
    }

    /// 列出所有工具名称
    pub fn list_names(&self) -> Vec<String> {
        let mut names = self.tools.keys().cloned().collect::<Vec<_>>();

        // 添加技能中的工具
        for skill in self.skills.list_enabled() {
            for tool in skill.tools() {
                if !names.contains(&tool.name) {
                    names.push(tool.name.clone());
                }
            }
        }

        names.sort();
        names.dedup();
        names
    }

    /// 获取所有 MCP 工具
    pub fn get_all_mcp_tools(&self) -> Vec<McpTool> {
        let mut tools = Vec::new();

        // 独立注册的工具
        for tool in self.tools.values() {
            tools.push(tool.to_mcp_tool());
        }

        // 技能中的工具
        for skill in self.skills.list_enabled() {
            tools.extend(skill.tools().to_vec());
        }

        tools
    }

    /// 执行工具调用
    pub async fn call(&self, call: &McpToolCall) -> AgentResult<McpToolResult> {
        // 先检查是否是技能工具
        if let Some(skill) = self.skills.find_skill_for_tool(&call.name) {
            return skill.execute_tool(call).await;
        }

        // 查找普通工具
        let tool = self
            .get(&call.name)
            .ok_or_else(|| AgentError::Skill(format!("Tool '{}' not found", call.name)))?;

        tool.execute(call.arguments.clone()).await
    }

    /// 批量执行工具调用
    pub async fn call_batch(&self, calls: &[McpToolCall]) -> AgentResult<Vec<McpToolResult>> {
        let mut results = Vec::new();

        for call in calls {
            let result = self.call(call).await?;
            results.push(result);
        }

        Ok(results)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 工具构建器
pub struct ToolBuilder {
    name: String,
    description: String,
    input_schema: Value,
}

impl ToolBuilder {
    /// 创建构建器
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    /// 设置描述
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// 设置输入 Schema
    pub fn input_schema(mut self, schema: Value) -> Self {
        self.input_schema = schema;
        self
    }

    /// 添加参数
    pub fn add_param(
        mut self,
        name: impl Into<String>,
        param_type: impl Into<String>,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        let param_name = name.into();
        let properties = self.input_schema["properties"]
            .as_object_mut()
            .unwrap();

        properties.insert(
            param_name.clone(),
            json!({
                "type": param_type.into(),
                "description": description.into()
            }),
        );

        if required {
            let required_array = self.input_schema["required"]
                .as_array_mut()
                .unwrap();
            required_array.push(serde_json::Value::String(param_name));
        }

        self
    }

    /// 构建工具
    pub fn build<F>(self, executor: F) -> Tool
    where
        F: Fn(Value) -> AgentResult<McpToolResult> + Send + Sync + 'static,
    {
        Tool::new(
            self.name,
            self.description,
            self.input_schema,
            Arc::new(executor),
        )
    }
}

/// 常用工具定义
pub mod builtin_tools {
    use super::*;
    use chrono::Utc;

    /// 获取当前时间
    pub fn get_current_time_tool() -> Tool {
        ToolBuilder::new("get_current_time")
            .description("Get the current date and time")
            .build(|_args| {
                Ok(McpToolResult {
                    name: "get_current_time".to_string(),
                    content: vec![McpContent::Text {
                        text: format!("Current time: {}", Utc::now().to_rfc3339()),
                    }],
                    is_error: false,
                })
            })
    }

    /// 计算器
    pub fn calculator_tool() -> Tool {
        ToolBuilder::new("calculator")
            .description("Perform basic arithmetic calculations")
            .add_param("expression", "string", "Mathematical expression to evaluate", true)
            .build(|args| {
                let expression = args["expression"]
                    .as_str()
                    .ok_or_else(|| AgentError::InvalidConfig("Expression must be a string".to_string()))?;

                // 简单的计算器（生产环境应该使用更安全的实现）
                let result = eval_expression(expression)?;

                Ok(McpToolResult {
                    name: "calculator".to_string(),
                    content: vec![McpContent::Text {
                        text: format!("{} = {}", expression, result),
                    }],
                    is_error: false,
                })
            })
    }

    /// 文本搜索
    pub fn text_search_tool() -> Tool {
        ToolBuilder::new("text_search")
            .description("Search for text in documents")
            .add_param("query", "string", "Search query", true)
            .add_param("path", "string", "File or directory path", true)
            .build(|args| {
                let query = args["query"].as_str().unwrap_or("");
                let path = args["path"].as_str().unwrap_or(".");

                // 简化实现，实际应该进行文件搜索
                Ok(McpToolResult {
                    name: "text_search".to_string(),
                    content: vec![McpContent::Text {
                        text: format!("Searched for '{}' in '{}'", query, path),
                    }],
                    is_error: false,
                })
            })
    }
}

/// 简单表达式求值
fn eval_expression(expr: &str) -> AgentResult<f64> {
    // 这是一个非常简化的实现，仅用于演示
    // 生产环境应该使用更安全和强大的表达式求值库

    // 移除空格
    let expr = expr.replace(" ", "");

    // 简单的加法
    if let Some((left, right)) = expr.split_once('+') {
        let left_val: f64 = left.parse().map_err(|_| {
            AgentError::InvalidConfig(format!("Invalid number: {}", left))
        })?;
        let right_val: f64 = right.parse().map_err(|_| {
            AgentError::InvalidConfig(format!("Invalid number: {}", right))
        })?;
        return Ok(left_val + right_val);
    }

    // 简单的减法
    if let Some((left, right)) = expr.split_once('-') {
        let left_val: f64 = left.parse().map_err(|_| {
            AgentError::InvalidConfig(format!("Invalid number: {}", left))
        })?;
        let right_val: f64 = right.parse().map_err(|_| {
            AgentError::InvalidConfig(format!("Invalid number: {}", right))
        })?;
        return Ok(left_val - right_val);
    }

    // 尝试直接解析为数字
    expr.parse().map_err(|_| {
        AgentError::InvalidConfig(format!("Unsupported expression: {}", expr))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tool_registry() {
        let mut registry = ToolRegistry::new();

        registry.register(builtin_tools::get_current_time_tool());

        let call = McpToolCall {
            name: "get_current_time".to_string(),
            arguments: json!({}),
        };

        let result = registry.call(&call).await.unwrap();
        assert_eq!(result.name, "get_current_time");
        assert!(!result.is_error);
    }

    #[test]
    fn test_tool_builder() {
        let tool = ToolBuilder::new("test_tool")
            .description("Test tool")
            .add_param("arg1", "string", "First argument", true)
            .add_param("arg2", "number", "Second argument", false)
            .build(|_args| {
                Ok(McpToolResult {
                    name: "test_tool".to_string(),
                    content: vec![McpContent::Text {
                        text: "Test result".to_string(),
                    }],
                    is_error: false,
                })
            });

        assert_eq!(tool.name, "test_tool");
    }
}
