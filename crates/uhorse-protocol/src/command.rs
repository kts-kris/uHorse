//! 命令定义
//!
//! 定义 Hub 发送给 Node 的各类命令

use crate::types::CommandType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// 节点可执行的命令
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    /// 文件操作
    File(FileCommand),

    /// Shell 命令
    Shell(ShellCommand),

    /// 代码执行
    Code(CodeCommand),

    /// 数据库查询
    Database(DatabaseCommand),

    /// API 调用
    Api(ApiCommand),

    /// 浏览器操作
    Browser(BrowserCommand),

    /// 自定义技能执行
    Skill(SkillCommand),
}

impl Command {
    /// 获取命令类型
    pub fn command_type(&self) -> CommandType {
        match self {
            Self::File(_) => CommandType::File,
            Self::Shell(_) => CommandType::Shell,
            Self::Code(_) => CommandType::Code,
            Self::Database(_) => CommandType::Database,
            Self::Api(_) => CommandType::Api,
            Self::Browser(_) => CommandType::Browser,
            Self::Skill(_) => CommandType::Skill,
        }
    }

    /// 估算执行时间（秒）
    pub fn estimated_duration(&self) -> Duration {
        match self {
            Self::File(cmd) => cmd.estimated_duration(),
            Self::Shell(cmd) => cmd.estimated_duration(),
            Self::Code(cmd) => cmd.timeout,
            Self::Database(cmd) => cmd.timeout,
            Self::Api(cmd) => cmd.timeout,
            Self::Browser(cmd) => cmd.timeout(),
            Self::Skill(cmd) => cmd.timeout,
        }
    }

    /// 获取所需权限
    pub fn required_permissions(&self) -> Vec<String> {
        match self {
            Self::File(cmd) => cmd.required_permissions(),
            Self::Shell(cmd) => cmd.required_permissions(),
            Self::Code(_) => vec!["code:execute".to_string()],
            Self::Database(_) => vec!["database:query".to_string()],
            Self::Api(_) => vec!["api:call".to_string()],
            Self::Browser(_) => vec!["browser:operate".to_string()],
            Self::Skill(cmd) => vec![format!("skill:execute:{}", cmd.skill_name)],
        }
    }
}

// ============================================================================
// 文件操作命令
// ============================================================================

/// 文件操作命令
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum FileCommand {
    /// 读取文件
    Read {
        /// 文件路径
        path: String,
        /// 读取限制
        limit: Option<usize>,
        /// 偏移量
        offset: Option<usize>,
    },

    /// 写入文件
    Write {
        /// 文件路径
        path: String,
        /// 文件内容
        content: String,
        /// 是否覆盖
        overwrite: bool,
    },

    /// 追加内容
    Append {
        /// 文件路径
        path: String,
        /// 追加内容
        content: String,
    },

    /// 删除文件/目录
    Delete {
        /// 路径
        path: String,
        /// 是否递归删除
        recursive: bool,
    },

    /// 列出目录
    List {
        /// 目录路径
        path: String,
        /// 是否递归
        recursive: bool,
        /// 过滤模式
        pattern: Option<String>,
    },

    /// 搜索文件
    Search {
        /// 搜索模式
        pattern: String,
        /// 搜索路径
        path: String,
        /// 是否递归
        recursive: bool,
        /// 搜索内容
        content_pattern: Option<String>,
    },

    /// 复制文件
    Copy {
        /// 源路径
        from: String,
        /// 目标路径
        to: String,
        /// 是否覆盖
        overwrite: bool,
    },

    /// 移动文件
    Move {
        /// 源路径
        from: String,
        /// 目标路径
        to: String,
        /// 是否覆盖
        overwrite: bool,
    },

    /// 获取文件信息
    Info {
        /// 文件路径
        path: String,
    },

    /// 创建目录
    CreateDir {
        /// 目录路径
        path: String,
        /// 是否递归创建
        recursive: bool,
    },

    /// 检查文件是否存在
    Exists {
        /// 文件路径
        path: String,
    },
}

impl FileCommand {
    /// 获取目标路径
    pub fn target_path(&self) -> &str {
        match self {
            Self::Read { path, .. } => path,
            Self::Write { path, .. } => path,
            Self::Append { path, .. } => path,
            Self::Delete { path, .. } => path,
            Self::List { path, .. } => path,
            Self::Search { path, .. } => path,
            Self::Copy { from, .. } => from,
            Self::Move { from, .. } => from,
            Self::Info { path, .. } => path,
            Self::CreateDir { path, .. } => path,
            Self::Exists { path, .. } => path,
        }
    }

    /// 估算执行时间
    pub fn estimated_duration(&self) -> Duration {
        match self {
            Self::Read { .. } => Duration::from_millis(100),
            Self::Write { .. } => Duration::from_millis(100),
            Self::Append { .. } => Duration::from_millis(50),
            Self::Delete { .. } => Duration::from_millis(200),
            Self::List { recursive, .. } => {
                if *recursive {
                    Duration::from_secs(5)
                } else {
                    Duration::from_millis(500)
                }
            }
            Self::Search { .. } => Duration::from_secs(10),
            Self::Copy { .. } => Duration::from_secs(2),
            Self::Move { .. } => Duration::from_millis(500),
            Self::Info { .. } => Duration::from_millis(50),
            Self::CreateDir { .. } => Duration::from_millis(50),
            Self::Exists { .. } => Duration::from_millis(20),
        }
    }

    /// 获取所需权限
    pub fn required_permissions(&self) -> Vec<String> {
        match self {
            Self::Read { .. } => vec!["file:read".to_string()],
            Self::Write { .. } => vec!["file:write".to_string()],
            Self::Append { .. } => vec!["file:write".to_string()],
            Self::Delete { .. } => vec!["file:delete".to_string()],
            Self::List { .. } => vec!["file:read".to_string()],
            Self::Search { .. } => vec!["file:read".to_string()],
            Self::Copy { .. } => vec!["file:read".to_string(), "file:write".to_string()],
            Self::Move { .. } => vec!["file:read".to_string(), "file:write".to_string(), "file:delete".to_string()],
            Self::Info { .. } => vec!["file:read".to_string()],
            Self::CreateDir { .. } => vec!["file:write".to_string()],
            Self::Exists { .. } => vec!["file:read".to_string()],
        }
    }
}

// ============================================================================
// Shell 命令
// ============================================================================

/// Shell 命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellCommand {
    /// 命令
    pub command: String,

    /// 参数
    pub args: Vec<String>,

    /// 工作目录
    pub cwd: Option<String>,

    /// 环境变量
    pub env: HashMap<String, String>,

    /// 超时时间
    #[serde(with = "duration_ser")]
    pub timeout: Duration,

    /// 是否捕获标准错误
    pub capture_stderr: bool,
}

impl ShellCommand {
    /// 创建新的 Shell 命令
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            cwd: None,
            env: HashMap::new(),
            timeout: Duration::from_secs(60),
            capture_stderr: true,
        }
    }

    /// 添加参数
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// 添加多个参数
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args.extend(args);
        self
    }

    /// 设置工作目录
    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// 设置环境变量
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 估算执行时间
    pub fn estimated_duration(&self) -> Duration {
        self.timeout
    }

    /// 获取所需权限
    pub fn required_permissions(&self) -> Vec<String> {
        vec!["shell:execute".to_string()]
    }
}

// ============================================================================
// 代码执行命令
// ============================================================================

/// 代码语言
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodeLanguage {
    /// Python
    Python,
    /// JavaScript
    JavaScript,
    /// TypeScript
    TypeScript,
    /// Rust
    Rust,
    /// Go
    Go,
    /// Shell
    Shell,
    /// SQL
    Sql,
}

/// 代码执行命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeCommand {
    /// 语言
    pub language: CodeLanguage,

    /// 代码内容
    pub code: String,

    /// 入口文件（用于多文件项目）
    pub entry_file: Option<String>,

    /// 依赖项
    pub dependencies: Vec<String>,

    /// 超时时间
    #[serde(with = "duration_ser")]
    pub timeout: Duration,

    /// 环境变量
    pub env: HashMap<String, String>,

    /// 工作目录
    pub workdir: Option<String>,
}

impl CodeCommand {
    /// 创建新的代码执行命令
    pub fn new(language: CodeLanguage, code: impl Into<String>) -> Self {
        Self {
            language,
            code: code.into(),
            entry_file: None,
            dependencies: Vec::new(),
            timeout: Duration::from_secs(60),
            env: HashMap::new(),
            workdir: None,
        }
    }

    /// 设置入口文件
    pub fn with_entry(mut self, entry: impl Into<String>) -> Self {
        self.entry_file = Some(entry.into());
        self
    }

    /// 添加依赖
    pub fn with_dependency(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

// ============================================================================
// 数据库命令
// ============================================================================

/// 数据库类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    /// PostgreSQL
    Postgres,
    /// MySQL
    Mysql,
    /// SQLite
    Sqlite,
    /// MongoDB
    Mongodb,
    /// Redis
    Redis,
}

/// 数据库查询命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseCommand {
    /// 数据库类型
    pub db_type: DatabaseType,

    /// 连接名称（引用预配置的连接）
    pub connection_name: Option<String>,

    /// 连接字符串（直接指定）
    pub connection_string: Option<String>,

    /// 查询语句
    pub query: String,

    /// 参数
    pub params: Vec<serde_json::Value>,

    /// 超时时间
    #[serde(with = "duration_ser")]
    pub timeout: Duration,

    /// 最大返回行数
    pub limit: Option<usize>,
}

impl DatabaseCommand {
    /// 创建新的数据库查询命令
    pub fn new(db_type: DatabaseType, query: impl Into<String>) -> Self {
        Self {
            db_type,
            connection_name: None,
            connection_string: None,
            query: query.into(),
            params: Vec::new(),
            timeout: Duration::from_secs(30),
            limit: Some(1000),
        }
    }

    /// 使用预配置连接
    pub fn with_connection(mut self, name: impl Into<String>) -> Self {
        self.connection_name = Some(name.into());
        self
    }

    /// 直接指定连接字符串
    pub fn with_connection_string(mut self, conn: impl Into<String>) -> Self {
        self.connection_string = Some(conn.into());
        self
    }

    /// 添加参数
    pub fn with_param(mut self, param: serde_json::Value) -> Self {
        self.params.push(param);
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 设置返回行数限制
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

// ============================================================================
// API 命令
// ============================================================================

/// HTTP 方法
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// GET
    Get,
    /// POST
    Post,
    /// PUT
    Put,
    /// DELETE
    Delete,
    /// PATCH
    Patch,
    /// HEAD
    Head,
}

/// API 调用命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCommand {
    /// HTTP 方法
    pub method: HttpMethod,

    /// URL
    pub url: String,

    /// 请求头
    pub headers: HashMap<String, String>,

    /// 查询参数
    pub query: HashMap<String, String>,

    /// 请求体
    pub body: Option<serde_json::Value>,

    /// 超时时间
    #[serde(with = "duration_ser")]
    pub timeout: Duration,

    /// 是否验证 SSL
    pub verify_ssl: bool,
}

impl ApiCommand {
    /// 创建新的 API 调用命令
    pub fn new(method: HttpMethod, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: HashMap::new(),
            query: HashMap::new(),
            body: None,
            timeout: Duration::from_secs(30),
            verify_ssl: true,
        }
    }

    /// GET 请求
    pub fn get(url: impl Into<String>) -> Self {
        Self::new(HttpMethod::Get, url)
    }

    /// POST 请求
    pub fn post(url: impl Into<String>) -> Self {
        Self::new(HttpMethod::Post, url)
    }

    /// 添加请求头
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// 添加查询参数
    pub fn with_query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.insert(key.into(), value.into());
        self
    }

    /// 设置请求体
    pub fn with_body(mut self, body: serde_json::Value) -> Self {
        self.body = Some(body);
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

// ============================================================================
// 浏览器命令
// ============================================================================

/// 浏览器操作命令
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BrowserCommand {
    /// 打开页面
    Navigate {
        /// URL
        url: String,
    },

    /// 截图
    Screenshot {
        /// 选择器（可选）
        selector: Option<String>,
        /// 全页面
        full_page: bool,
    },

    /// 点击元素
    Click {
        /// 选择器
        selector: String,
    },

    /// 输入文本
    Type {
        /// 选择器
        selector: String,
        /// 文本
        text: String,
    },

    /// 等待元素
    WaitFor {
        /// 选择器
        selector: String,
        /// 超时时间（秒）
        timeout_secs: u64,
    },

    /// 获取文本
    GetText {
        /// 选择器
        selector: String,
    },

    /// 执行脚本
    Evaluate {
        /// JavaScript 代码
        script: String,
    },

    /// 关闭页面
    Close,
}

impl BrowserCommand {
    /// 超时时间
    pub fn timeout(&self) -> Duration {
        match self {
            Self::Navigate { .. } => Duration::from_secs(30),
            Self::Screenshot { .. } => Duration::from_secs(10),
            Self::Click { .. } => Duration::from_secs(5),
            Self::Type { .. } => Duration::from_secs(5),
            Self::WaitFor { timeout_secs, .. } => Duration::from_secs(*timeout_secs),
            Self::GetText { .. } => Duration::from_secs(5),
            Self::Evaluate { .. } => Duration::from_secs(10),
            Self::Close => Duration::from_secs(5),
        }
    }
}

// ============================================================================
// 技能命令
// ============================================================================

/// 技能执行命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCommand {
    /// 技能名称
    pub skill_name: String,

    /// 技能版本
    pub version: Option<String>,

    /// 输入参数
    pub input: serde_json::Value,

    /// 超时时间
    #[serde(with = "duration_ser")]
    pub timeout: Duration,

    /// 执行选项
    pub options: HashMap<String, serde_json::Value>,
}

impl SkillCommand {
    /// 创建新的技能执行命令
    pub fn new(skill_name: impl Into<String>, input: serde_json::Value) -> Self {
        Self {
            skill_name: skill_name.into(),
            version: None,
            input,
            timeout: Duration::from_secs(60),
            options: HashMap::new(),
        }
    }

    /// 设置版本
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 添加选项
    pub fn with_option(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.options.insert(key.into(), value);
        self
    }
}

// ============================================================================
// Duration 序列化
// ============================================================================

mod duration_ser {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}
