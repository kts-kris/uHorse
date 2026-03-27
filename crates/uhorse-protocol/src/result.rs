//! 执行结果定义
//!
//! 定义命令执行后的结果格式

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 命令执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// 是否成功
    pub success: bool,

    /// 输出内容
    pub output: CommandOutput,

    /// 退出码
    pub exit_code: Option<i32>,

    /// 执行时间（毫秒）
    pub duration_ms: u64,

    /// 资源使用
    pub resources: ResourceUsage,

    /// 错误信息（如果失败）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ExecutionError>,

    /// 警告信息
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,

    /// 附加元数据
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl CommandResult {
    /// 创建成功结果
    pub fn success(output: CommandOutput) -> Self {
        Self {
            success: true,
            output,
            exit_code: Some(0),
            duration_ms: 0,
            resources: ResourceUsage::default(),
            error: None,
            warnings: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// 创建失败结果
    pub fn failure(error: ExecutionError) -> Self {
        Self {
            success: false,
            output: CommandOutput::None,
            exit_code: Some(1),
            duration_ms: 0,
            resources: ResourceUsage::default(),
            error: Some(error),
            warnings: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// 设置执行时间
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    /// 设置资源使用
    pub fn with_resources(mut self, resources: ResourceUsage) -> Self {
        self.resources = resources;
        self
    }

    /// 添加警告
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// 命令输出
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandOutput {
    /// 文本输出
    Text {
        /// 文本内容
        content: String,
    },

    /// JSON 输出
    Json {
        /// JSON 内容
        content: serde_json::Value,
    },

    /// 二进制输出（引用）
    Binary {
        /// MIME 类型
        mime_type: String,
        /// 大小（字节）
        size: usize,
        /// 存储引用（用于获取实际数据）
        storage_ref: String,
    },

    /// 文件列表
    FileList {
        /// 文件列表
        files: Vec<FileInfo>,
    },

    /// 数据库结果
    Database {
        /// 列名
        columns: Vec<String>,
        /// 行数据
        rows: Vec<Vec<serde_json::Value>>,
        /// 总行数（可能与返回行数不同，如果有限制）
        total_rows: Option<usize>,
    },

    /// API 响应
    ApiResponse {
        /// HTTP 状态码
        status: u16,
        /// 响应头
        headers: HashMap<String, String>,
        /// 响应体
        body: Option<serde_json::Value>,
    },

    /// 浏览器结果
    Browser {
        /// 操作结果
        result: BrowserResult,
    },

    /// 技能执行结果
    Skill {
        /// 技能名称
        skill_name: String,
        /// 输出
        output: serde_json::Value,
    },

    /// 空（无输出）
    None,
}

impl CommandOutput {
    /// 创建文本输出
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text {
            content: content.into(),
        }
    }

    /// 创建 JSON 输出
    pub fn json(content: serde_json::Value) -> Self {
        Self::Json { content }
    }

    /// 创建文件列表输出
    pub fn file_list(files: Vec<FileInfo>) -> Self {
        Self::FileList { files }
    }

    /// 创建数据库结果输出
    pub fn database(columns: Vec<String>, rows: Vec<Vec<serde_json::Value>>) -> Self {
        Self::Database {
            columns,
            rows: rows.clone(),
            total_rows: Some(rows.len()),
        }
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::None)
    }

    /// 获取文本内容（如果是文本输出）
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { content } => Some(content),
            _ => None,
        }
    }

    /// 获取 JSON 内容（如果是 JSON 输出）
    pub fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Json { content } => Some(content),
            _ => None,
        }
    }
}

/// 文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    /// 文件路径
    pub path: String,

    /// 文件名
    pub name: String,

    /// 是否为目录
    pub is_dir: bool,

    /// 文件大小（字节）
    pub size: u64,

    /// 修改时间
    pub modified: Option<DateTime<Utc>>,

    /// 创建时间
    pub created: Option<DateTime<Utc>>,

    /// 权限（Unix 模式）
    pub mode: Option<u32>,

    /// MIME 类型
    pub mime_type: Option<String>,

    /// 扩展属性
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, serde_json::Value>,
}

impl FileInfo {
    /// 创建新的文件信息
    pub fn new(path: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            name: name.into(),
            is_dir: false,
            size: 0,
            modified: None,
            created: None,
            mode: None,
            mime_type: None,
            extra: HashMap::new(),
        }
    }

    /// 设置为目录
    pub fn as_dir(mut self) -> Self {
        self.is_dir = true;
        self
    }

    /// 设置大小
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }

    /// 设置修改时间
    pub fn with_modified(mut self, modified: DateTime<Utc>) -> Self {
        self.modified = Some(modified);
        self
    }
}

/// 资源使用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU 使用率 (0.0 - 100.0)
    pub cpu_percent: f32,

    /// 内存使用 (MB)
    pub memory_mb: u64,

    /// 峰值内存 (MB)
    pub peak_memory_mb: u64,

    /// 磁盘读取 (KB)
    pub disk_read_kb: u64,

    /// 磁盘写入 (KB)
    pub disk_write_kb: u64,

    /// 网络发送 (KB)
    pub network_sent_kb: u64,

    /// 网络接收 (KB)
    pub network_recv_kb: u64,
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            memory_mb: 0,
            peak_memory_mb: 0,
            disk_read_kb: 0,
            disk_write_kb: 0,
            network_sent_kb: 0,
            network_recv_kb: 0,
        }
    }
}

/// 浏览器操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserResult {
    /// 系统浏览器打开结果
    OpenSystem {
        /// 打开的 URL
        url: String,
    },

    /// 导航结果
    Navigate {
        /// 最终 URL（可能有重定向）
        final_url: String,
        /// 页面标题
        title: Option<String>,
    },

    /// 截图结果
    Screenshot {
        /// 图片格式
        format: String,
        /// 图片数据（Base64）
        data: String,
        /// 宽度
        width: u32,
        /// 高度
        height: u32,
    },

    /// 文本获取结果
    GetText {
        /// 文本内容
        text: String,
    },

    /// 脚本执行结果
    Evaluate {
        /// 返回值
        value: serde_json::Value,
    },

    /// 简单确认
    Ok,

    /// 错误
    Error {
        /// 错误消息
        message: String,
    },
}

/// 执行错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionError {
    /// 错误代码
    pub code: String,

    /// 错误消息
    pub message: String,

    /// 错误详情
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,

    /// 错误来源
    pub source: ErrorSource,

    /// 是否可重试
    #[serde(default)]
    pub retryable: bool,

    /// 重试建议等待时间（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

impl ExecutionError {
    /// 创建新的执行错误
    pub fn new(code: impl Into<String>, message: impl Into<String>, source: ErrorSource) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            source,
            retryable: false,
            retry_after_ms: None,
        }
    }

    /// 设置详情
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// 设置可重试
    pub fn with_retryable(mut self, retry_after_ms: u64) -> Self {
        self.retryable = true;
        self.retry_after_ms = Some(retry_after_ms);
        self
    }

    /// 权限错误
    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::new("PERMISSION_DENIED", message, ErrorSource::Permission)
    }

    /// 超时错误
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new("TIMEOUT", message, ErrorSource::Timeout).with_retryable(1000)
    }

    /// 资源不足错误
    pub fn resource_exhausted(message: impl Into<String>) -> Self {
        Self::new("RESOURCE_EXHAUSTED", message, ErrorSource::Resource)
    }

    /// 执行失败
    pub fn execution_failed(message: impl Into<String>) -> Self {
        Self::new("EXECUTION_FAILED", message, ErrorSource::Executor)
    }

    /// 验证失败
    pub fn validation_failed(message: impl Into<String>) -> Self {
        Self::new("VALIDATION_FAILED", message, ErrorSource::Validation)
    }
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for ExecutionError {}

/// 错误来源
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorSource {
    /// 命令解析
    Parser,
    /// 权限检查
    Permission,
    /// 验证
    Validation,
    /// 执行器
    Executor,
    /// 超时
    Timeout,
    /// 资源
    Resource,
    /// 外部服务
    External,
    /// 内部错误
    Internal,
}

/// 进度更新
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    /// 进度 (0.0 - 1.0)
    pub progress: f32,

    /// 进度消息
    pub message: String,

    /// 当前步骤
    pub current_step: Option<String>,

    /// 总步骤数
    pub total_steps: Option<usize>,

    /// 当前步骤索引
    pub current_step_index: Option<usize>,

    /// 预计剩余时间（秒）
    pub eta_seconds: Option<u64>,

    /// 时间戳
    pub timestamp: DateTime<Utc>,
}

impl ProgressUpdate {
    /// 创建新的进度更新
    pub fn new(progress: f32, message: impl Into<String>) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            message: message.into(),
            current_step: None,
            total_steps: None,
            current_step_index: None,
            eta_seconds: None,
            timestamp: Utc::now(),
        }
    }

    /// 设置步骤信息
    pub fn with_step(mut self, step: impl Into<String>, index: usize, total: usize) -> Self {
        self.current_step = Some(step.into());
        self.current_step_index = Some(index);
        self.total_steps = Some(total);
        self
    }

    /// 设置预计剩余时间
    pub fn with_eta(mut self, seconds: u64) -> Self {
        self.eta_seconds = Some(seconds);
        self
    }
}

/// 批量操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    /// 总操作数
    pub total: usize,

    /// 成功数
    pub succeeded: usize,

    /// 失败数
    pub failed: usize,

    /// 跳过数
    pub skipped: usize,

    /// 各操作结果
    pub results: Vec<BatchItemResult>,
}

impl BatchResult {
    /// 创建新的批量结果
    pub fn new() -> Self {
        Self {
            total: 0,
            succeeded: 0,
            failed: 0,
            skipped: 0,
            results: Vec::new(),
        }
    }

    /// 添加成功结果
    pub fn add_success(&mut self, item: impl Into<String>) {
        self.total += 1;
        self.succeeded += 1;
        self.results.push(BatchItemResult {
            item: item.into(),
            success: true,
            error: None,
        });
    }

    /// 添加失败结果
    pub fn add_failure(&mut self, item: impl Into<String>, error: impl Into<String>) {
        self.total += 1;
        self.failed += 1;
        self.results.push(BatchItemResult {
            item: item.into(),
            success: false,
            error: Some(error.into()),
        });
    }

    /// 添加跳过结果
    pub fn add_skipped(&mut self, item: impl Into<String>, reason: impl Into<String>) {
        self.total += 1;
        self.skipped += 1;
        self.results.push(BatchItemResult {
            item: item.into(),
            success: true,
            error: Some(format!("Skipped: {}", reason.into())),
        });
    }

    /// 获取成功率
    pub fn success_rate(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.succeeded as f32 / self.total as f32
        }
    }
}

impl Default for BatchResult {
    fn default() -> Self {
        Self::new()
    }
}

/// 批量操作单项结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItemResult {
    /// 操作项
    pub item: String,

    /// 是否成功
    pub success: bool,

    /// 错误信息
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_result() {
        let result = CommandResult::success(CommandOutput::text("Hello"))
            .with_duration(100)
            .with_warning("Test warning");

        assert!(result.success);
        assert_eq!(result.duration_ms, 100);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_execution_error() {
        let error = ExecutionError::permission_denied("Access denied");
        assert_eq!(error.code, "PERMISSION_DENIED");
        assert!(!error.retryable);
    }

    #[test]
    fn test_batch_result() {
        let mut batch = BatchResult::new();
        batch.add_success("file1.txt");
        batch.add_failure("file2.txt", "Permission denied");

        assert_eq!(batch.total, 2);
        assert_eq!(batch.succeeded, 1);
        assert_eq!(batch.failed, 1);
    }
}
