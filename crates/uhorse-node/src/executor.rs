//! 命令执行器
//!
//! 负责执行 Hub 下发的各类命令

use crate::error::{NodeError, NodeResult};
use crate::permission::{PermissionManager, PermissionResult};
use crate::workspace::Workspace;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::process::Command as TokioCommand;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uhorse_protocol::{
    ApiCommand, BrowserCommand, CodeCommand, CodeLanguage, Command, CommandOutput,
    CommandResult as ProtocolCommandResult, DatabaseCommand, ExecutionError, FileCommand,
    HttpMethod, ShellCommand, SkillCommand, TaskContext, TaskId,
};

/// 命令执行器
#[derive(Debug)]
pub struct CommandExecutor {
    /// 工作空间
    workspace: Arc<Workspace>,

    /// 权限管理器
    permission_manager: Arc<PermissionManager>,

    /// 执行超时
    default_timeout: Duration,

    /// 最大输出大小
    max_output_size: usize,

    /// 执行统计
    stats: Arc<RwLock<ExecutorStats>>,
}

/// 执行器统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutorStats {
    /// 总执行次数
    pub total_executions: u64,

    /// 成功次数
    pub successful_executions: u64,

    /// 失败次数
    pub failed_executions: u64,

    /// 超时次数
    pub timeout_count: u64,

    /// 平均执行时间 (ms)
    pub avg_duration_ms: f64,

    /// 总执行时间 (ms)
    pub total_duration_ms: u64,
}

impl CommandExecutor {
    /// 创建新的命令执行器
    pub fn new(workspace: Arc<Workspace>, permission_manager: Arc<PermissionManager>) -> Self {
        Self {
            workspace,
            permission_manager,
            default_timeout: Duration::from_secs(60),
            max_output_size: 10 * 1024 * 1024, // 10 MB
            stats: Arc::new(RwLock::new(ExecutorStats::default())),
        }
    }

    /// 执行命令
    pub async fn execute(
        &self,
        task_id: &TaskId,
        command: &Command,
        context: &TaskContext,
    ) -> NodeResult<ProtocolCommandResult> {
        let start = Instant::now();

        // 1. 检查权限
        match self.permission_manager.check(command, context).await {
            PermissionResult::Allowed => {}
            PermissionResult::Denied(reason) => {
                return Ok(ProtocolCommandResult::failure(
                    ExecutionError::permission_denied(&reason),
                ));
            }
            PermissionResult::RequiresApproval { request_id, reason } => {
                // 返回需要审批的结果
                let error = ExecutionError::permission_denied(&format!(
                    "Operation requires approval. Request ID: {}, Reason: {}",
                    request_id, reason
                ))
                .with_retryable(1000); // 1秒后可重试

                return Ok(ProtocolCommandResult::failure(error));
            }
        }

        // 2. 执行命令
        let result = match command {
            Command::File(cmd) => self.execute_file(cmd).await,
            Command::Shell(cmd) => self.execute_shell(cmd).await,
            Command::Code(cmd) => self.execute_code(cmd).await,
            Command::Database(cmd) => self.execute_database(cmd).await,
            Command::Api(cmd) => self.execute_api(cmd).await,
            Command::Browser(cmd) => self.execute_browser(cmd).await,
            Command::Skill(cmd) => self.execute_skill(cmd).await,
        };

        // 3. 更新统计
        let duration = start.elapsed();
        {
            let mut stats = self.stats.write().await;
            stats.total_executions += 1;
            stats.total_duration_ms += duration.as_millis() as u64;
            stats.avg_duration_ms = stats.total_duration_ms as f64 / stats.total_executions as f64;
            if result.is_ok() {
                stats.successful_executions += 1;
            } else {
                stats.failed_executions += 1;
            }
        }

        // 4. 返回结果
        result.map(|output| {
            ProtocolCommandResult::success(output).with_duration(duration.as_millis() as u64)
        })
    }

    /// 执行文件操作
    async fn execute_file(&self, cmd: &FileCommand) -> NodeResult<CommandOutput> {
        match cmd {
            FileCommand::Read {
                path,
                limit,
                offset,
            } => self.file_read(path, *limit, *offset).await,
            FileCommand::Write {
                path,
                content,
                overwrite,
            } => self.file_write(path, content, *overwrite).await,
            FileCommand::Append { path, content } => self.file_append(path, content).await,
            FileCommand::Delete { path, recursive } => self.file_delete(path, *recursive).await,
            FileCommand::List {
                path,
                recursive,
                pattern,
            } => self.file_list(path, *recursive, pattern.as_deref()).await,
            FileCommand::Search {
                pattern,
                path,
                recursive,
                content_pattern,
            } => {
                self.file_search(pattern, path, *recursive, content_pattern.as_deref())
                    .await
            }
            FileCommand::Copy {
                from,
                to,
                overwrite,
            } => self.file_copy(from, to, *overwrite).await,
            FileCommand::Move {
                from,
                to,
                overwrite,
            } => self.file_move(from, to, *overwrite).await,
            FileCommand::Info { path } => self.file_info(path).await,
            FileCommand::CreateDir { path, recursive } => {
                self.file_create_dir(path, *recursive).await
            }
            FileCommand::Exists { path } => self.file_exists(path).await,
        }
    }

    /// 读取文件
    async fn file_read(
        &self,
        path: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> NodeResult<CommandOutput> {
        let path = Path::new(path);

        // 检查访问权限
        if !self.workspace.can_access(path, false)? {
            return Err(NodeError::Permission(format!(
                "Cannot read file: {:?}",
                path
            )));
        }

        // 读取文件
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to read file {:?}: {}", path, e)))?;

        // 应用偏移和限制
        let content = if let Some(offset) = offset {
            let start = content
                .char_indices()
                .nth(offset)
                .map(|(i, _)| i)
                .unwrap_or(content.len());
            content[start..].to_string()
        } else {
            content
        };

        let content = if let Some(limit) = limit {
            let end = content
                .char_indices()
                .nth(limit)
                .map(|(i, _)| i)
                .unwrap_or(content.len());
            content[..end].to_string()
        } else {
            content
        };

        Ok(CommandOutput::text(content))
    }

    /// 写入文件
    async fn file_write(
        &self,
        path: &str,
        content: &str,
        overwrite: bool,
    ) -> NodeResult<CommandOutput> {
        let path = Path::new(path);

        // 检查访问权限
        if !self.workspace.can_access(path, true)? {
            return Err(NodeError::Permission(format!(
                "Cannot write to file: {:?}",
                path
            )));
        }

        // 检查文件是否存在
        if path.exists() && !overwrite {
            return Err(NodeError::Execution(format!(
                "File already exists: {:?}",
                path
            )));
        }

        // 写入文件
        tokio::fs::write(path, content)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to write file {:?}: {}", path, e)))?;

        Ok(CommandOutput::text(format!(
            "Written {} bytes",
            content.len()
        )))
    }

    /// 追加内容
    async fn file_append(&self, path: &str, content: &str) -> NodeResult<CommandOutput> {
        let path = Path::new(path);

        // 检查访问权限
        if !self.workspace.can_access(path, true)? {
            return Err(NodeError::Permission(format!(
                "Cannot append to file: {:?}",
                path
            )));
        }

        // 追加内容
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to open file {:?}: {}", path, e)))?;

        file.write_all(content.as_bytes()).await.map_err(|e| {
            NodeError::Execution(format!("Failed to append to file {:?}: {}", path, e))
        })?;

        Ok(CommandOutput::text(format!(
            "Appended {} bytes",
            content.len()
        )))
    }

    /// 删除文件/目录
    async fn file_delete(&self, path: &str, recursive: bool) -> NodeResult<CommandOutput> {
        let path = Path::new(path);

        // 检查访问权限
        if !self.workspace.can_access(path, true)? {
            return Err(NodeError::Permission(format!("Cannot delete: {:?}", path)));
        }

        if path.is_dir() {
            if recursive {
                tokio::fs::remove_dir_all(path).await.map_err(|e| {
                    NodeError::Execution(format!("Failed to delete directory {:?}: {}", path, e))
                })?;
            } else {
                tokio::fs::remove_dir(path).await.map_err(|e| {
                    NodeError::Execution(format!("Failed to delete directory {:?}: {}", path, e))
                })?;
            }
        } else {
            tokio::fs::remove_file(path).await.map_err(|e| {
                NodeError::Execution(format!("Failed to delete file {:?}: {}", path, e))
            })?;
        }

        Ok(CommandOutput::text("Deleted successfully"))
    }

    /// 列出目录内容
    async fn file_list(
        &self,
        path: &str,
        recursive: bool,
        pattern: Option<&str>,
    ) -> NodeResult<CommandOutput> {
        let path = Path::new(path);

        // 检查访问权限
        if !self.workspace.can_access(path, false)? {
            return Err(NodeError::Permission(format!(
                "Cannot list directory: {:?}",
                path
            )));
        }

        let mut entries = Vec::new();

        if recursive {
            self.list_recursive(path, pattern, &mut entries)?;
        } else {
            let mut dir = tokio::fs::read_dir(path).await.map_err(|e| {
                NodeError::Execution(format!("Failed to read directory {:?}: {}", path, e))
            })?;

            while let Some(entry) = dir
                .next_entry()
                .await
                .map_err(|e| NodeError::Execution(format!("Failed to read entry: {}", e)))?
            {
                let entry_path = entry.path();
                let name = entry_path.file_name().unwrap_or_default().to_string_lossy();

                if let Some(p) = pattern {
                    if !glob_match::glob_match(p, &name) {
                        continue;
                    }
                }

                let metadata = entry.metadata().await.ok();
                entries.push(serde_json::json!({
                    "name": name,
                    "path": entry_path.to_string_lossy(),
                    "is_dir": metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                    "size": metadata.as_ref().map(|m| m.len()).unwrap_or(0),
                }));
            }
        }

        Ok(CommandOutput::json(
            serde_json::json!({ "entries": entries }),
        ))
    }

    /// 递归列出目录
    fn list_recursive(
        &self,
        path: &Path,
        pattern: Option<&str>,
        entries: &mut Vec<serde_json::Value>,
    ) -> NodeResult<()> {
        let dir = std::fs::read_dir(path).map_err(|e| {
            NodeError::Execution(format!("Failed to read directory {:?}: {}", path, e))
        })?;

        for entry in dir {
            let entry =
                entry.map_err(|e| NodeError::Execution(format!("Failed to read entry: {}", e)))?;

            let entry_path = entry.path();
            let name = entry_path.file_name().unwrap_or_default().to_string_lossy();

            if let Some(p) = pattern {
                if !glob_match::glob_match(p, &name) {
                    continue;
                }
            }

            let metadata = entry.metadata().ok();
            entries.push(serde_json::json!({
                "name": name,
                "path": entry_path.to_string_lossy(),
                "is_dir": metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                "size": metadata.as_ref().map(|m| m.len()).unwrap_or(0),
            }));

            if entry_path.is_dir() {
                self.list_recursive(&entry_path, pattern, entries)?;
            }
        }

        Ok(())
    }

    /// 搜索文件
    async fn file_search(
        &self,
        pattern: &str,
        path: &str,
        recursive: bool,
        content_pattern: Option<&str>,
    ) -> NodeResult<CommandOutput> {
        let path = Path::new(path);

        // 检查访问权限
        if !self.workspace.can_access(path, false)? {
            return Err(NodeError::Permission(format!(
                "Cannot search in: {:?}",
                path
            )));
        }

        let mut results = Vec::new();

        if recursive {
            self.search_recursive(path, pattern, content_pattern, &mut results)?;
        } else {
            self.search_single_dir(path, pattern, content_pattern, &mut results)?;
        }

        Ok(CommandOutput::json(
            serde_json::json!({ "results": results }),
        ))
    }

    /// 递归搜索
    fn search_recursive(
        &self,
        path: &Path,
        pattern: &str,
        content_pattern: Option<&str>,
        results: &mut Vec<serde_json::Value>,
    ) -> NodeResult<()> {
        let dir = std::fs::read_dir(path).map_err(|e| {
            NodeError::Execution(format!("Failed to read directory {:?}: {}", path, e))
        })?;

        for entry in dir {
            let entry =
                entry.map_err(|e| NodeError::Execution(format!("Failed to read entry: {}", e)))?;

            let entry_path = entry.path();
            let name = entry_path.file_name().unwrap_or_default().to_string_lossy();

            if glob_match::glob_match(pattern, &name) {
                if let Some(content_pattern) = content_pattern {
                    if entry_path.is_file() {
                        if let Ok(content) = std::fs::read_to_string(&entry_path) {
                            if content.contains(content_pattern) {
                                results.push(serde_json::json!({
                                    "path": entry_path.to_string_lossy(),
                                    "name": name,
                                    "matched_content": true,
                                }));
                            }
                        }
                    }
                } else {
                    results.push(serde_json::json!({
                        "path": entry_path.to_string_lossy(),
                        "name": name,
                    }));
                }
            }

            if entry_path.is_dir() {
                self.search_recursive(&entry_path, pattern, content_pattern, results)?;
            }
        }

        Ok(())
    }

    /// 单目录搜索
    fn search_single_dir(
        &self,
        path: &Path,
        pattern: &str,
        content_pattern: Option<&str>,
        results: &mut Vec<serde_json::Value>,
    ) -> NodeResult<()> {
        let dir = std::fs::read_dir(path).map_err(|e| {
            NodeError::Execution(format!("Failed to read directory {:?}: {}", path, e))
        })?;

        for entry in dir {
            let entry =
                entry.map_err(|e| NodeError::Execution(format!("Failed to read entry: {}", e)))?;

            let entry_path = entry.path();
            let name = entry_path.file_name().unwrap_or_default().to_string_lossy();

            if glob_match::glob_match(pattern, &name) {
                if let Some(content_pattern) = content_pattern {
                    if entry_path.is_file() {
                        if let Ok(content) = std::fs::read_to_string(&entry_path) {
                            if content.contains(content_pattern) {
                                results.push(serde_json::json!({
                                    "path": entry_path.to_string_lossy(),
                                    "name": name,
                                    "matched_content": true,
                                }));
                            }
                        }
                    }
                } else {
                    results.push(serde_json::json!({
                        "path": entry_path.to_string_lossy(),
                        "name": name,
                    }));
                }
            }
        }

        Ok(())
    }

    /// 复制文件
    async fn file_copy(&self, from: &str, to: &str, overwrite: bool) -> NodeResult<CommandOutput> {
        let from = Path::new(from);
        let to = Path::new(to);

        // 检查访问权限
        if !self.workspace.can_access(from, false)? {
            return Err(NodeError::Permission(format!(
                "Cannot read from: {:?}",
                from
            )));
        }
        if !self.workspace.can_access(to, true)? {
            return Err(NodeError::Permission(format!("Cannot write to: {:?}", to)));
        }

        // 检查目标是否存在
        if to.exists() && !overwrite {
            return Err(NodeError::Execution(format!(
                "Destination already exists: {:?}",
                to
            )));
        }

        // 复制文件
        tokio::fs::copy(from, to)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to copy: {}", e)))?;

        Ok(CommandOutput::text("Copied successfully"))
    }

    /// 移动文件
    async fn file_move(&self, from: &str, to: &str, overwrite: bool) -> NodeResult<CommandOutput> {
        let from = Path::new(from);
        let to = Path::new(to);

        // 检查访问权限
        if !self.workspace.can_access(from, true)? {
            return Err(NodeError::Permission(format!(
                "Cannot move from: {:?}",
                from
            )));
        }
        if !self.workspace.can_access(to, true)? {
            return Err(NodeError::Permission(format!("Cannot move to: {:?}", to)));
        }

        // 检查目标是否存在
        if to.exists() && !overwrite {
            return Err(NodeError::Execution(format!(
                "Destination already exists: {:?}",
                to
            )));
        }

        // 移动文件
        tokio::fs::rename(from, to)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to move: {}", e)))?;

        Ok(CommandOutput::text("Moved successfully"))
    }

    /// 获取文件信息
    async fn file_info(&self, path: &str) -> NodeResult<CommandOutput> {
        let info = self.workspace.get_file_info(path)?;
        Ok(CommandOutput::json(serde_json::to_value(info)?))
    }

    /// 创建目录
    async fn file_create_dir(&self, path: &str, recursive: bool) -> NodeResult<CommandOutput> {
        let path = Path::new(path);

        // 检查访问权限
        if !self.workspace.can_access(path, true)? {
            return Err(NodeError::Permission(format!(
                "Cannot create directory: {:?}",
                path
            )));
        }

        if recursive {
            tokio::fs::create_dir_all(path).await.map_err(|e| {
                NodeError::Execution(format!("Failed to create directory {:?}: {}", path, e))
            })?;
        } else {
            tokio::fs::create_dir(path).await.map_err(|e| {
                NodeError::Execution(format!("Failed to create directory {:?}: {}", path, e))
            })?;
        }

        Ok(CommandOutput::text("Directory created"))
    }

    /// 检查文件是否存在
    async fn file_exists(&self, path: &str) -> NodeResult<CommandOutput> {
        let exists = Path::new(path).exists();
        Ok(CommandOutput::json(serde_json::json!({ "exists": exists })))
    }

    /// 执行 Shell 命令
    async fn execute_shell(&self, cmd: &ShellCommand) -> NodeResult<CommandOutput> {
        debug!("Executing shell command: {} {:?}", cmd.command, cmd.args);

        let mut command = TokioCommand::new(&cmd.command);
        command.args(&cmd.args);

        if let Some(cwd) = &cmd.cwd {
            command.current_dir(cwd);
        }

        for (key, value) in &cmd.env {
            command.env(key, value);
        }

        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        // 执行命令
        let output = command
            .output()
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to execute command: {}", e)))?;

        // 构建输出
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            if cmd.capture_stderr && !stderr.is_empty() {
                Ok(CommandOutput::text(format!("{}\n{}", stdout, stderr)))
            } else {
                Ok(CommandOutput::text(stdout))
            }
        } else {
            Err(NodeError::Execution(format!(
                "Command failed with exit code {:?}: {}",
                output.status.code(),
                stderr
            )))
        }
    }

    /// 执行代码
    async fn execute_code(&self, cmd: &CodeCommand) -> NodeResult<CommandOutput> {
        let lang_str = format!("{:?}", cmd.language);
        info!("Executing {} code", lang_str);

        match cmd.language {
            CodeLanguage::Python => self.execute_python(cmd).await,
            CodeLanguage::JavaScript | CodeLanguage::TypeScript => {
                self.execute_javascript(cmd).await
            }
            CodeLanguage::Shell => {
                let shell_cmd = ShellCommand {
                    command: "sh".to_string(),
                    args: vec!["-c".to_string(), cmd.code.clone()],
                    cwd: cmd.workdir.clone(),
                    env: HashMap::new(),
                    timeout: cmd.timeout,
                    capture_stderr: true,
                };

                self.execute_shell(&shell_cmd).await
            }
            CodeLanguage::Sql => Err(NodeError::Execution(
                "SQL should be executed via Database command".to_string(),
            )),
            CodeLanguage::Rust | CodeLanguage::Go => Err(NodeError::Execution(format!(
                "{:?} execution not yet supported",
                cmd.language
            ))),
        }
    }

    /// 执行 Python 代码
    async fn execute_python(&self, cmd: &CodeCommand) -> NodeResult<CommandOutput> {
        // 创建临时文件
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("uhorse_code_{}.py", uuid::Uuid::new_v4()));

        // 同步写入文件
        std::fs::write(&temp_path, &cmd.code)
            .map_err(|e| NodeError::Execution(format!("Failed to write temp file: {}", e)))?;

        let path = temp_path.to_string_lossy().to_string();

        let shell_cmd = ShellCommand {
            command: "python3".to_string(),
            args: vec![path.clone()],
            cwd: cmd.workdir.clone(),
            env: HashMap::new(),
            timeout: cmd.timeout,
            capture_stderr: true,
        };

        let result = self.execute_shell(&shell_cmd).await;

        // 清理临时文件
        let _ = std::fs::remove_file(&temp_path);

        result
    }

    /// 执行 JavaScript 代码
    async fn execute_javascript(&self, cmd: &CodeCommand) -> NodeResult<CommandOutput> {
        let ext = if matches!(cmd.language, CodeLanguage::TypeScript) {
            ".ts"
        } else {
            ".js"
        };

        // 创建临时文件
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("uhorse_code_{}{}", uuid::Uuid::new_v4(), ext));

        // 同步写入文件
        std::fs::write(&temp_path, &cmd.code)
            .map_err(|e| NodeError::Execution(format!("Failed to write temp file: {}", e)))?;

        let path = temp_path.to_string_lossy().to_string();

        let runner = if matches!(cmd.language, CodeLanguage::TypeScript) {
            "ts-node"
        } else {
            "node"
        };

        let shell_cmd = ShellCommand {
            command: runner.to_string(),
            args: vec![path.clone()],
            cwd: cmd.workdir.clone(),
            env: HashMap::new(),
            timeout: cmd.timeout,
            capture_stderr: true,
        };

        let result = self.execute_shell(&shell_cmd).await;

        // 清理临时文件
        let _ = std::fs::remove_file(&temp_path);

        result
    }

    /// 执行数据库查询
    async fn execute_database(&self, _cmd: &DatabaseCommand) -> NodeResult<CommandOutput> {
        // 数据库执行需要更复杂的实现
        Err(NodeError::Execution(
            "Database execution not yet implemented".to_string(),
        ))
    }

    /// 执行 API 调用
    async fn execute_api(&self, cmd: &ApiCommand) -> NodeResult<CommandOutput> {
        let method_str = format!("{:?}", cmd.method);
        info!("Making {} request to {}", method_str, cmd.url);

        let client = reqwest::Client::builder()
            .timeout(cmd.timeout)
            .danger_accept_invalid_certs(!cmd.verify_ssl)
            .build()
            .map_err(|e| NodeError::Execution(format!("Failed to create HTTP client: {}", e)))?;

        let mut request = match cmd.method {
            HttpMethod::Get => client.get(&cmd.url),
            HttpMethod::Post => client.post(&cmd.url),
            HttpMethod::Put => client.put(&cmd.url),
            HttpMethod::Delete => client.delete(&cmd.url),
            HttpMethod::Patch => client.patch(&cmd.url),
            HttpMethod::Head => client.head(&cmd.url),
        };

        // 添加请求头
        for (key, value) in &cmd.headers {
            request = request.header(key, value);
        }

        // 添加查询参数
        for (key, value) in &cmd.query {
            request = request.query(&[(key, value)]);
        }

        // 添加请求体
        if let Some(body) = &cmd.body {
            request = request.json(body);
        }

        let response = request
            .send()
            .await
            .map_err(|e| NodeError::Execution(format!("API request failed: {}", e)))?;

        let status = response.status().as_u16();
        let headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let body = response.json::<serde_json::Value>().await.ok();

        Ok(CommandOutput::ApiResponse {
            status,
            headers,
            body,
        })
    }

    /// 执行浏览器操作
    async fn execute_browser(&self, _cmd: &BrowserCommand) -> NodeResult<CommandOutput> {
        // 浏览器执行需要 headless browser 集成
        Err(NodeError::Execution(
            "Browser execution not yet implemented".to_string(),
        ))
    }

    /// 执行技能
    async fn execute_skill(&self, _cmd: &SkillCommand) -> NodeResult<CommandOutput> {
        // 技能执行需要技能系统
        Err(NodeError::Execution(
            "Skill execution not yet implemented".to_string(),
        ))
    }

    /// 获取统计
    pub async fn get_stats(&self) -> ExecutorStats {
        self.stats.read().await.clone()
    }
}

/// 执行上下文（用于日志和追踪）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// 任务 ID
    pub task_id: String,

    /// 用户 ID
    pub user_id: String,

    /// 会话 ID
    pub session_id: String,

    /// 开始时间
    pub started_at: DateTime<Utc>,

    /// 结束时间
    pub finished_at: Option<DateTime<Utc>>,

    /// 命令类型
    pub command_type: String,

    /// 是否成功
    pub success: Option<bool>,

    /// 错误信息
    pub error: Option<String>,
}

impl ExecutionContext {
    /// 创建新的执行上下文
    pub fn new(task_id: impl Into<String>, command_type: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            user_id: String::new(),
            session_id: String::new(),
            started_at: Utc::now(),
            finished_at: None,
            command_type: command_type.into(),
            success: None,
            error: None,
        }
    }

    /// 设置用户 ID
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = user_id.into();
        self
    }

    /// 设置会话 ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = session_id.into();
        self
    }

    /// 标记完成
    pub fn complete(mut self, success: bool, error: Option<String>) -> Self {
        self.finished_at = Some(Utc::now());
        self.success = Some(success);
        self.error = error;
        self
    }
}
