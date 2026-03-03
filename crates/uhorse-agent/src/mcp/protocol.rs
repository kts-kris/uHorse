//! # MCP 协议处理
//!
//! MCP (Model Context Protocol) 协议实现。

use crate::error::{AgentError, AgentResult};
use crate::mcp::types::*;
use serde_json::json;

/// MCP 协议处理器
pub struct McpProtocol;

impl McpProtocol {
    /// 创建初始化响应
    pub fn initialize(id: serde_json::Value, server_info: McpServerInfo) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "protocolVersion": server_info.protocol_version,
                "serverInfo": {
                    "name": server_info.name,
                    "version": server_info.version
                },
                "capabilities": {
                    "tools": true,
                    "resources": true,
                    "prompts": true
                }
            })),
            error: None,
        }
    }

    /// 创建工具列表响应
    pub fn tools_list(id: serde_json::Value, tools: Vec<McpTool>) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "tools": tools
            })),
            error: None,
        }
    }

    /// 创建工具调用响应
    pub fn tools_call(id: serde_json::Value, result: McpToolResult) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "content": result.content,
                "isError": result.is_error
            })),
            error: None,
        }
    }

    /// 创建资源列表响应
    pub fn resources_list(id: serde_json::Value, resources: Vec<McpResource>) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "resources": resources
            })),
            error: None,
        }
    }

    /// 创建资源读取响应
    pub fn resources_read(id: serde_json::Value, contents: McpResourceContents) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "contents": [{
                    "uri": contents.uri,
                    "text": contents.contents
                }]
            })),
            error: None,
        }
    }

    /// 创建提示词列表响应
    pub fn prompts_list(id: serde_json::Value, prompts: Vec<McpPrompt>) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "prompts": prompts
            })),
            error: None,
        }
    }

    /// 创建错误响应
    pub fn error(id: serde_json::Value, code: i32, message: &str) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(McpError {
                code,
                message: message.to_string(),
                data: None,
            }),
        }
    }

    /// 解析 MCP 请求
    pub fn parse_request(json: &str) -> AgentResult<McpRequest> {
        serde_json::from_str(json)
            .map_err(|e| AgentError::InvalidConfig(format!("Failed to parse MCP request: {}", e)))
    }

    /// 序列化 MCP 响应
    pub fn serialize_response(response: &McpResponse) -> AgentResult<String> {
        serde_json::to_string(response).map_err(|e| {
            AgentError::InvalidConfig(format!("Failed to serialize MCP response: {}", e))
        })
    }
}

/// MCP 错误代码
pub mod error_codes {
    /// 解析错误
    pub const PARSE_ERROR: i32 = -32700;
    /// 无效请求
    pub const INVALID_REQUEST: i32 = -32600;
    /// 方法未找到
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// 无效参数
    pub const INVALID_PARAMS: i32 = -32602;
    /// 内部错误
    pub const INTERNAL_ERROR: i32 = -32603;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_response() {
        let server_info = McpServerInfo {
            name: "Test Server".to_string(),
            version: "1.0.0".to_string(),
            protocol_version: "2024-11-05".to_string(),
        };

        let response = McpProtocol::initialize(json!(1), server_info);

        assert_eq!(response.id, json!(1));
        assert!(response.error.is_none());
        assert!(response.result.is_some());
    }

    #[test]
    fn test_error_response() {
        let response =
            McpProtocol::error(json!(1), error_codes::METHOD_NOT_FOUND, "Method not found");

        assert_eq!(response.id, json!(1));
        assert!(response.result.is_none());
        assert!(response.error.is_some());
    }
}
