//! # 内置工具集
//!
//! 提供常用的内置工具实现。

use openclaw_core::{ToolExecutor, ExecutionContext, Result, ToolId, PermissionLevel};
use async_trait::async_trait;

// ============== 计算器工具 ==============

/// 计算器工具 - 支持基本数学运算
#[derive(Debug)]
pub struct CalculatorTool {
    id: ToolId,
}

impl CalculatorTool {
    pub fn new() -> Self {
        Self {
            id: ToolId("calculator".to_string()),
        }
    }
}

impl Default for CalculatorTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ToolExecutor for CalculatorTool {
    fn id(&self) -> &ToolId {
        &self.id
    }

    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Perform basic mathematical calculations (+, -, *, /)"
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        &*Box::leak(Box::new(serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Mathematical expression (e.g., '2 + 2', '10 * 5')"
                }
            },
            "required": ["expression"]
        })))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Public
    }

    async fn execute(&self, params: serde_json::Value, _context: &ExecutionContext) -> Result<serde_json::Value> {
        let expr = params["expression"]
            .as_str()
            .ok_or_else(|| openclaw_core::OpenClawError::ToolValidationFailed("Missing 'expression' parameter".to_string()))?;

        // 简单的计算器实现
        let result = self.evaluate_expression(expr)
            .map_err(|e| openclaw_core::OpenClawError::ToolExecutionFailed(format!("Calculation error: {}", e)))?;

        Ok(serde_json::json!({
            "expression": expr,
            "result": result
        }))
    }
}

impl CalculatorTool {
    /// 简单的表达式求值（仅支持基本运算）
    fn evaluate_expression(&self, expr: &str) -> Result<f64, String> {
        // 移除空格
        let expr = expr.replace(" ", "");

        // 简单解析：支持 a + b, a - b, a * b, a / b 格式
        if let Some(pos) = expr.find('+') {
            let left: f64 = expr[..pos].parse().map_err(|_| "Invalid left operand".to_string())?;
            let right: f64 = expr[pos+1..].parse().map_err(|_| "Invalid right operand".to_string())?;
            return Ok(left + right);
        }
        if let Some(pos) = expr.find('-') {
            let left: f64 = expr[..pos].parse().map_err(|_| "Invalid left operand".to_string())?;
            let right: f64 = expr[pos+1..].parse().map_err(|_| "Invalid right operand".to_string())?;
            return Ok(left - right);
        }
        if let Some(pos) = expr.find('*') {
            let left: f64 = expr[..pos].parse().map_err(|_| "Invalid left operand".to_string())?;
            let right: f64 = expr[pos+1..].parse().map_err(|_| "Invalid right operand".to_string())?;
            return Ok(left * right);
        }
        if let Some(pos) = expr.find('/') {
            let left: f64 = expr[..pos].parse().map_err(|_| "Invalid left operand".to_string())?;
            let right: f64 = expr[pos+1..].parse().map_err(|_| "Invalid right operand".to_string())?;
            if right == 0.0 {
                return Err("Division by zero".to_string());
            }
            return Ok(left / right);
        }

        // 尝试直接解析为数字
        expr.parse::<f64>().map_err(|_| "Invalid expression".to_string())
    }
}

// ============== HTTP 请求工具 ==============

/// HTTP 请求工具 - 发起 HTTP 请求
#[derive(Debug)]
pub struct HttpTool {
    id: ToolId,
    client: reqwest::Client,
}

impl HttpTool {
    pub fn new() -> Self {
        Self {
            id: ToolId("http".to_string()),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        self
    }
}

impl Default for HttpTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ToolExecutor for HttpTool {
    fn id(&self) -> &ToolId {
        &self.id
    }

    fn name(&self) -> &str {
        "http"
    }

    fn description(&self) -> &str {
        "Make HTTP requests to external APIs"
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        &*Box::leak(Box::new(serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Target URL"
                },
                "method": {
                    "type": "string",
                    "enum": ["GET", "POST", "PUT", "DELETE"],
                    "description": "HTTP method"
                },
                "headers": {
                    "type": "object",
                    "description": "Request headers"
                },
                "body": {
                    "description": "Request body (for POST/PUT)"
                }
            },
            "required": ["url", "method"]
        })))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Authenticated
    }

    async fn execute(&self, params: serde_json::Value, _context: &ExecutionContext) -> Result<serde_json::Value> {
        let url = params["url"]
            .as_str()
            .ok_or_else(|| openclaw_core::OpenClawError::ToolValidationFailed("Missing 'url' parameter".to_string()))?;

        let method_str = params["method"]
            .as_str()
            .ok_or_else(|| openclaw_core::OpenClawError::ToolValidationFailed("Missing 'method' parameter".to_string()))?;

        let method = match method_str {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "DELETE" => reqwest::Method::DELETE,
            _ => return Err(openclaw_core::OpenClawError::ToolValidationFailed("Invalid HTTP method".to_string())),
        };

        let mut request = self.client.request(method, url);

        // 添加 headers
        if let Some(headers) = params.get("headers").and_then(|v| v.as_object()) {
            for (key, value) in headers {
                if let Some(str_val) = value.as_str() {
                    request = request.header(key, str_val);
                }
            }
        }

        // 添加 body
        if let Some(body) = params.get("body") {
            request = request.json(body);
        }

        let response = request
            .send()
            .await
            .map_err(|e| openclaw_core::OpenClawError::ToolExecutionFailed(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        let headers: std::collections::HashMap<String, String> = response
            .headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
            .collect();

        let body_text = response
            .text()
            .await
            .map_err(|e| openclaw_core::OpenClawError::ToolExecutionFailed(format!("Failed to read response: {}", e)))?;

        Ok(serde_json::json!({
            "status": status.as_u16(),
            "headers": headers,
            "body": body_text
        }))
    }
}

// ============== 搜索工具 ==============

/// Web 搜索工具
#[derive(Debug)]
pub struct SearchTool {
    id: ToolId,
}

impl SearchTool {
    pub fn new() -> Self {
        Self {
            id: ToolId("search".to_string()),
        }
    }
}

impl Default for SearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ToolExecutor for SearchTool {
    fn id(&self) -> &ToolId {
        &self.id
    }

    fn name(&self) -> &str {
        "search"
    }

    fn description(&self) -> &str {
        "Search the web for information"
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        &*Box::leak(Box::new(serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results",
                    "default": 5
                }
            },
            "required": ["query"]
        })))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Public
    }

    async fn execute(&self, params: serde_json::Value, _context: &ExecutionContext) -> Result<serde_json::Value> {
        let query = params["query"]
            .as_str()
            .ok_or_else(|| openclaw_core::OpenClawError::ToolValidationFailed("Missing 'query' parameter".to_string()))?;

        let limit = params["limit"]
            .as_u64()
            .unwrap_or(5) as usize;

        // TODO: 集成真实的搜索 API
        // 这里返回模拟结果
        Ok(serde_json::json!({
            "query": query,
            "results": [],
            "note": "Search API integration pending"
        }))
    }
}

// ============== 日期时间工具 ==============

/// 日期时间工具
#[derive(Debug)]
pub struct DatetimeTool {
    id: ToolId,
}

impl DatetimeTool {
    pub fn new() -> Self {
        Self {
            id: ToolId("datetime".to_string()),
        }
    }
}

impl Default for DatetimeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ToolExecutor for DatetimeTool {
    fn id(&self) -> &ToolId {
        &self.id
    }

    fn name(&self) -> &str {
        "datetime"
    }

    fn description(&self) -> &str {
        "Get current date and time, or parse/format dates"
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        &*Box::leak(Box::new(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["now", "parse", "format"],
                    "description": "Action to perform"
                },
                "input": {
                    "type": "string",
                    "description": "Input date string (for parse/format)"
                },
                "format": {
                    "type": "string",
                    "description": "Date format string (e.g., %Y-%m-%d)"
                }
            }
        })))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Public
    }

    async fn execute(&self, params: serde_json::Value, _context: &ExecutionContext) -> Result<serde_json::Value> {
        let action = params["action"].as_str().unwrap_or("now");

        match action {
            "now" => {
                let now = chrono::Utc::now();
                Ok(serde_json::json!({
                    "iso": now.to_rfc3339(),
                    "timestamp": now.timestamp(),
                    "unix": now.timestamp()
                }))
            }
            "parse" => {
                let input = params["input"]
                    .as_str()
                    .ok_or_else(|| openclaw_core::OpenClawError::ToolValidationFailed("Missing 'input' for parse".to_string()))?;

                // 尝试解析常见格式
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(input) {
                    Ok(serde_json::json!({
                        "parsed": dt.to_rfc3339(),
                        "timestamp": dt.timestamp()
                    }))
                } else {
                    Ok(serde_json::json!({
                        "error": "Unable to parse date",
                        "input": input
                    }))
                }
            }
            "format" => {
                let input = params["input"]
                    .as_str()
                    .ok_or_else(|| openclaw_core::OpenClawError::ToolValidationFailed("Missing 'input' for format".to_string()))?;

                let format_str = params["format"].as_str().unwrap_or("%Y-%m-%d %H:%M:%S");

                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(input) {
                    Ok(serde_json::json!({
                        "formatted": dt.format(format_str).to_string()
                    }))
                } else {
                    Ok(serde_json::json!({
                        "error": "Unable to parse date for formatting"
                    }))
                }
            }
            _ => Err(openclaw_core::OpenClawError::ToolValidationFailed(format!("Unknown action: {}", action)))
        }
    }
}

// ============== 文本处理工具 ==============

/// 文本处理工具
#[derive(Debug)]
pub struct TextTool {
    id: ToolId,
}

impl TextTool {
    pub fn new() -> Self {
        Self {
            id: ToolId("text".to_string()),
        }
    }
}

impl Default for TextTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ToolExecutor for TextTool {
    fn id(&self) -> &ToolId {
        &self.id
    }

    fn name(&self) -> &str {
        "text"
    }

    fn description(&self) -> &str {
        "Text manipulation utilities (count, split, join, etc.)"
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        &*Box::leak(Box::new(serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["count", "split", "join", "upper", "lower", "reverse"],
                    "description": "Action to perform"
                },
                "text": {
                    "type": "string",
                    "description": "Input text"
                },
                "separator": {
                    "type": "string",
                    "description": "Separator for split/join"
                }
            },
            "required": ["action", "text"]
        })))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Public
    }

    async fn execute(&self, params: serde_json::Value, _context: &ExecutionContext) -> Result<serde_json::Value> {
        let action = params["action"]
            .as_str()
            .ok_or_else(|| openclaw_core::OpenClawError::ToolValidationFailed("Missing 'action' parameter".to_string()))?;

        let text = params["text"]
            .as_str()
            .ok_or_else(|| openclaw_core::OpenClawError::ToolValidationFailed("Missing 'text' parameter".to_string()))?;

        let result = match action {
            "count" => serde_json::json!({ "count": text.chars().count() }),
            "split" => {
                let sep = params["separator"].as_str().unwrap_or(" ");
                serde_json::json!({ "parts": text.split(sep).collect::<Vec<_>>() })
            }
            "join" => {
                // 假设 text 是 JSON 数组
                if let Some(arr) = params["text"].as_array() {
                    let sep = params["separator"].as_str().unwrap_or(" ");
                    let joined: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                    serde_json::json!({ "result": joined.join(sep) })
                } else {
                    serde_json::json!({ "error": "text must be an array for join" })
                }
            }
            "upper" => serde_json::json!({ "result": text.to_uppercase() }),
            "lower" => serde_json::json!({ "result": text.to_lowercase() }),
            "reverse" => serde_json::json!({ "result": text.chars().rev().collect::<String>() }),
            _ => return Err(openclaw_core::OpenClawError::ToolValidationFailed(format!("Unknown action: {}", action)))
        };

        Ok(result)
    }
}
