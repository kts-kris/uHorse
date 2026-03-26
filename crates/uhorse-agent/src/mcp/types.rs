//! # MCP 核心类型定义
//!
//! 基于 MCP (Model Context Protocol) 规范的类型定义。

use serde::{Deserialize, Serialize};

/// MCP 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// 工具名称
    pub name: String,
    /// 工具描述（供 AI 阅读理解）
    pub description: String,
    /// JSON Schema 输入定义
    #[serde(alias = "inputSchema")]
    pub input_schema: serde_json::Value,
}

/// MCP 工具调用参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCall {
    /// 工具名称
    pub name: String,
    /// 调用参数（符合 input_schema）
    pub arguments: serde_json::Value,
}

/// MCP 工具调用结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    /// 工具名称
    pub name: String,
    /// 返回内容
    pub content: Vec<McpContent>,
    /// 是否有错误
    #[serde(default)]
    pub is_error: bool,
}

/// MCP 内容块
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpContent {
    /// 文本内容
    #[serde(rename = "text")]
    Text {
        /// 文本正文。
        text: String,
    },
    /// 图片内容
    #[serde(rename = "image")]
    Image {
        /// 图片数据。
        data: String,
        /// 图片 MIME 类型。
        mime_type: String,
    },
    /// 资源内容
    #[serde(rename = "resource")]
    Resource {
        /// 资源 URI。
        uri: String,
        /// 资源内容。
        contents: String,
    },
}

/// MCP 资源定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    /// 资源 URI（支持模板）
    pub uri: String,
    /// 资源名称
    pub name: String,
    /// 资源描述
    pub description: String,
    /// MIME 类型
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// MCP 资源内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceContents {
    /// 资源 URI
    pub uri: String,
    /// 内容
    pub contents: String,
}

/// MCP 提示词定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPrompt {
    /// 提示词名称
    pub name: String,
    /// 提示词描述
    pub description: String,
    /// 参数定义
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<McpPromptArgument>>,
}

/// MCP 提示词参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptArgument {
    /// 参数名称
    pub name: String,
    /// 参数描述
    pub description: String,
    /// 是否必需
    #[serde(default)]
    pub required: bool,
}

/// MCP 提示词消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptMessage {
    /// 角色
    pub role: String,
    /// 内容
    pub content: McpContent,
}

/// MCP 服务器信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    /// 服务器名称
    pub name: String,
    /// 服务器版本
    pub version: String,
    /// 协议版本
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
}

fn default_protocol_version() -> String {
    "2024-11-05".to_string()
}

/// MCP 能力声明
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpCapabilities {
    /// 支持的工具
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<bool>,
    /// 支持的资源
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<bool>,
    /// 支持的提示词
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<bool>,
}

/// MCP JSON-RPC 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    /// JSON-RPC 版本
    #[serde(default = "default_jsonrpc_version")]
    pub jsonrpc: String,
    /// 请求 ID
    pub id: serde_json::Value,
    /// 方法名
    pub method: String,
    /// 参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

fn default_jsonrpc_version() -> String {
    "2.0".to_string()
}

/// MCP JSON-RPC 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    /// JSON-RPC 版本
    #[serde(default = "default_jsonrpc_version")]
    pub jsonrpc: String,
    /// 请求 ID
    pub id: serde_json::Value,
    /// 结果
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// 错误
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP 错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    /// 错误代码
    pub code: i32,
    /// 错误消息
    pub message: String,
    /// 错误数据
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// MCP 方法名
pub mod methods {
    /// 初始化
    pub const INITIALIZE: &str = "initialize";
    /// 列出工具
    pub const TOOLS_LIST: &str = "tools/list";
    /// 调用工具
    pub const TOOLS_CALL: &str = "tools/call";
    /// 列出资源
    pub const RESOURCES_LIST: &str = "resources/list";
    /// 读取资源
    pub const RESOURCES_READ: &str = "resources/read";
    /// 列出提示词
    pub const PROMPTS_LIST: &str = "prompts/list";
    /// 获取提示词
    pub const PROMPTS_GET: &str = "prompts/get";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let tool = McpTool {
            name: "get_weather".to_string(),
            description: "Get current weather".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                },
                "required": ["city"]
            }),
        };

        assert_eq!(tool.name, "get_weather");
    }

    #[test]
    fn test_tool_call() {
        let call = McpToolCall {
            name: "get_weather".to_string(),
            arguments: serde_json::json!({"city": "Beijing"}),
        };

        assert_eq!(call.name, "get_weather");
    }
}
