//! 命令执行器
//!
//! 负责执行 Hub 下发的各类命令

use crate::browser::{BrowserConfig, BrowserExecutor};
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
use tracing::{debug, info};
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

    /// 内部工作目录
    internal_work_dir: String,

    /// 执行超时
    default_timeout: Duration,

    /// 最大输出大小
    max_output_size: usize,

    /// 执行统计
    stats: Arc<RwLock<ExecutorStats>>,

    /// 浏览器执行器
    browser_executor: BrowserExecutor,
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
    pub fn new(
        workspace: Arc<Workspace>,
        permission_manager: Arc<PermissionManager>,
        internal_work_dir: impl Into<String>,
    ) -> Self {
        Self {
            workspace,
            permission_manager,
            internal_work_dir: internal_work_dir.into(),
            default_timeout: Duration::from_secs(60),
            max_output_size: 10 * 1024 * 1024,
            stats: Arc::new(RwLock::new(ExecutorStats::default())),
            browser_executor: BrowserExecutor::new(BrowserConfig::default()),
        }
    }

    /// 执行命令
    pub async fn execute(
        &self,
        _task_id: &TaskId,
        command: &Command,
        context: &TaskContext,
    ) -> NodeResult<ProtocolCommandResult> {
        match self.permission_manager.check(command, context).await {
            PermissionResult::Allowed => self.execute_unchecked(command).await,
            PermissionResult::Denied(reason) => Ok(ProtocolCommandResult::failure(
                ExecutionError::permission_denied(&reason),
            )),
            PermissionResult::RequiresApproval { request_id, reason } => {
                let error = ExecutionError::permission_denied(format!(
                    "Operation requires approval. Request ID: {}, Reason: {}",
                    request_id, reason
                ))
                .with_retryable(1000);

                Ok(ProtocolCommandResult::failure(error))
            }
        }
    }

    /// 在权限已确认后执行命令
    pub async fn execute_unchecked(&self, command: &Command) -> NodeResult<ProtocolCommandResult> {
        let start = Instant::now();

        let result = match command {
            Command::File(cmd) => self.execute_file(cmd).await,
            Command::Shell(cmd) => self.execute_shell(cmd).await,
            Command::Code(cmd) => self.execute_code(cmd).await,
            Command::Database(cmd) => self.execute_database(cmd).await,
            Command::Api(cmd) => self.execute_api(cmd).await,
            Command::Browser(cmd) => self.execute_browser(cmd).await,
            Command::Skill(cmd) => self.execute_skill(cmd).await,
        };

        let duration = start.elapsed();
        {
            let mut stats = self.stats.write().await;
            stats.total_executions += 1;
            stats.total_duration_ms += duration.as_millis() as u64;
            stats.avg_duration_ms = stats.total_duration_ms as f64 / stats.total_executions as f64;
            match &result {
                Ok(_) => stats.successful_executions += 1,
                Err(NodeError::Timeout(_)) => {
                    stats.failed_executions += 1;
                    stats.timeout_count += 1;
                }
                Err(_) => stats.failed_executions += 1,
            }
        }

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
        let path = self.resolve_file_path(path, false)?;

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to read file {:?}: {}", path, e)))?;

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
        let path = self.resolve_file_path(path, true)?;

        if path.exists() && !overwrite {
            return Err(NodeError::Execution(format!(
                "File already exists: {:?}",
                path
            )));
        }

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                NodeError::Execution(format!(
                    "Failed to create parent directory {:?}: {}",
                    parent, e
                ))
            })?;
        }

        tokio::fs::write(&path, content)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to write file {:?}: {}", path, e)))?;

        Ok(CommandOutput::json(serde_json::json!({
            "kind": "file_operation",
            "action": "write",
            "path": path.to_string_lossy().to_string(),
            "bytes_written": content.len(),
        })))
    }

    /// 追加内容
    async fn file_append(&self, path: &str, content: &str) -> NodeResult<CommandOutput> {
        let path = self.resolve_file_path(path, true)?;

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                NodeError::Execution(format!(
                    "Failed to create parent directory {:?}: {}",
                    parent, e
                ))
            })?;
        }

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to open file {:?}: {}", path, e)))?;

        file.write_all(content.as_bytes()).await.map_err(|e| {
            NodeError::Execution(format!("Failed to append to file {:?}: {}", path, e))
        })?;

        Ok(CommandOutput::json(serde_json::json!({
            "kind": "file_operation",
            "action": "append",
            "path": path.to_string_lossy().to_string(),
            "bytes_appended": content.len(),
        })))
    }

    /// 删除文件/目录
    async fn file_delete(&self, path: &str, recursive: bool) -> NodeResult<CommandOutput> {
        let path = self.resolve_file_path(path, true)?;

        if path.is_dir() {
            if recursive {
                tokio::fs::remove_dir_all(&path).await.map_err(|e| {
                    NodeError::Execution(format!("Failed to delete directory {:?}: {}", path, e))
                })?;
            } else {
                tokio::fs::remove_dir(&path).await.map_err(|e| {
                    NodeError::Execution(format!("Failed to delete directory {:?}: {}", path, e))
                })?;
            }
        } else {
            tokio::fs::remove_file(&path).await.map_err(|e| {
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
        let path = self.resolve_file_path(path, false)?;
        let mut entries = Vec::new();

        if recursive {
            self.list_recursive(&path, pattern, &mut entries)?;
        } else {
            let mut dir = tokio::fs::read_dir(&path).await.map_err(|e| {
                NodeError::Execution(format!("Failed to read directory {:?}: {}", path, e))
            })?;

            while let Some(entry) = dir
                .next_entry()
                .await
                .map_err(|e| NodeError::Execution(format!("Failed to read entry: {}", e)))?
            {
                let entry_path = entry.path();
                if !self.workspace.can_access(&entry_path, false)? {
                    continue;
                }

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
            if !self.workspace.can_access(&entry_path, false)? {
                continue;
            }

            let name = entry_path.file_name().unwrap_or_default().to_string_lossy();
            if let Some(p) = pattern {
                if !glob_match::glob_match(p, &name) {
                    if entry_path.is_dir() {
                        self.list_recursive(&entry_path, pattern, entries)?;
                    }
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
        let path = self.resolve_file_path(path, false)?;
        let mut results = Vec::new();

        if recursive {
            self.search_recursive(&path, pattern, content_pattern, &mut results)?;
        } else {
            self.search_single_dir(&path, pattern, content_pattern, &mut results)?;
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
            if !self.workspace.can_access(&entry_path, false)? {
                continue;
            }

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
            if !self.workspace.can_access(&entry_path, false)? {
                continue;
            }

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
        let from = self.resolve_file_path(from, false)?;
        let to = self.resolve_file_path(to, true)?;

        if to.exists() && !overwrite {
            return Err(NodeError::Execution(format!(
                "Destination already exists: {:?}",
                to
            )));
        }

        if let Some(parent) = to.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                NodeError::Execution(format!(
                    "Failed to create parent directory {:?}: {}",
                    parent, e
                ))
            })?;
        }

        let copied_bytes = tokio::fs::copy(&from, &to)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to copy: {}", e)))?;

        Ok(CommandOutput::json(serde_json::json!({
            "kind": "file_operation",
            "action": "copy",
            "path": to.to_string_lossy().to_string(),
            "source_path": from.to_string_lossy().to_string(),
            "destination_path": to.to_string_lossy().to_string(),
            "bytes_copied": copied_bytes,
        })))
    }

    /// 移动文件
    async fn file_move(&self, from: &str, to: &str, overwrite: bool) -> NodeResult<CommandOutput> {
        let from = self.resolve_file_path(from, true)?;
        let to = self.resolve_file_path(to, true)?;

        if to.exists() && !overwrite {
            return Err(NodeError::Execution(format!(
                "Destination already exists: {:?}",
                to
            )));
        }

        if let Some(parent) = to.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                NodeError::Execution(format!(
                    "Failed to create parent directory {:?}: {}",
                    parent, e
                ))
            })?;
        }

        tokio::fs::rename(&from, &to)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to move: {}", e)))?;

        Ok(CommandOutput::json(serde_json::json!({
            "kind": "file_operation",
            "action": "move",
            "path": to.to_string_lossy().to_string(),
            "source_path": from.to_string_lossy().to_string(),
            "destination_path": to.to_string_lossy().to_string(),
        })))
    }

    /// 获取文件信息
    async fn file_info(&self, path: &str) -> NodeResult<CommandOutput> {
        let info = self.workspace.get_file_info(path)?;
        Ok(CommandOutput::json(serde_json::to_value(info)?))
    }

    /// 创建目录
    async fn file_create_dir(&self, path: &str, recursive: bool) -> NodeResult<CommandOutput> {
        let path = self.resolve_file_path(path, true)?;

        if recursive {
            tokio::fs::create_dir_all(&path).await.map_err(|e| {
                NodeError::Execution(format!("Failed to create directory {:?}: {}", path, e))
            })?;
        } else {
            tokio::fs::create_dir(&path).await.map_err(|e| {
                NodeError::Execution(format!("Failed to create directory {:?}: {}", path, e))
            })?;
        }

        Ok(CommandOutput::json(serde_json::json!({
            "kind": "file_operation",
            "action": "create_dir",
            "path": path.to_string_lossy().to_string(),
            "recursive": recursive,
        })))
    }

    /// 检查文件是否存在
    async fn file_exists(&self, path: &str) -> NodeResult<CommandOutput> {
        let path = self.resolve_file_path(path, false)?;
        let exists = tokio::fs::try_exists(&path)
            .await
            .map_err(|e| NodeError::Execution(format!("Failed to check file {:?}: {}", path, e)))?;
        Ok(CommandOutput::json(serde_json::json!({ "exists": exists })))
    }

    /// 执行 Shell 命令
    async fn execute_shell(&self, cmd: &ShellCommand) -> NodeResult<CommandOutput> {
        debug!("Executing shell command: {} {:?}", cmd.command, cmd.args);

        let cwd = self.resolve_execution_dir(cmd.cwd.as_deref())?;
        let mut command = TokioCommand::new(&cmd.command);
        command
            .args(&cmd.args)
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &cmd.env {
            command.env(key, value);
        }

        self.run_process(command, cmd.timeout).await
    }

    /// 执行代码
    async fn execute_code(&self, cmd: &CodeCommand) -> NodeResult<CommandOutput> {
        info!("Executing {:?} code", cmd.language);

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
                    env: cmd.env.clone(),
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
        let workdir = self.resolve_code_workdir(cmd)?;
        let temp_path = self.create_temp_code_file("py", &cmd.code).await?;

        let mut command = TokioCommand::new("python3");
        command
            .arg(&temp_path)
            .current_dir(&workdir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &cmd.env {
            command.env(key, value);
        }

        let result = self.run_process(command, cmd.timeout).await;
        let _ = tokio::fs::remove_file(&temp_path).await;
        result
    }

    /// 执行 JavaScript 代码
    async fn execute_javascript(&self, cmd: &CodeCommand) -> NodeResult<CommandOutput> {
        let workdir = self.resolve_code_workdir(cmd)?;
        let ext = if matches!(cmd.language, CodeLanguage::TypeScript) {
            "ts"
        } else {
            "js"
        };
        let temp_path = self.create_temp_code_file(ext, &cmd.code).await?;
        let runner = if matches!(cmd.language, CodeLanguage::TypeScript) {
            "ts-node"
        } else {
            "node"
        };

        let mut command = TokioCommand::new(runner);
        command
            .arg(&temp_path)
            .current_dir(&workdir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &cmd.env {
            command.env(key, value);
        }

        let result = self.run_process(command, cmd.timeout).await;
        let _ = tokio::fs::remove_file(&temp_path).await;
        result
    }

    /// 执行数据库查询
    async fn execute_database(&self, _cmd: &DatabaseCommand) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Database execution not yet implemented".to_string(),
        ))
    }

    /// 执行 API 调用
    async fn execute_api(&self, cmd: &ApiCommand) -> NodeResult<CommandOutput> {
        info!("Making {:?} request to {}", cmd.method, cmd.url);

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

        for (key, value) in &cmd.headers {
            request = request.header(key, value);
        }

        for (key, value) in &cmd.query {
            request = request.query(&[(key, value)]);
        }

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
    async fn execute_browser(&self, cmd: &BrowserCommand) -> NodeResult<CommandOutput> {
        self.browser_executor.execute(cmd).await
    }

    /// 执行技能
    async fn execute_skill(&self, _cmd: &SkillCommand) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Skill execution not yet implemented".to_string(),
        ))
    }

    /// 获取统计
    pub async fn get_stats(&self) -> ExecutorStats {
        self.stats.read().await.clone()
    }

    fn resolve_file_path(&self, path: &str, write: bool) -> NodeResult<PathBuf> {
        let resolved = self.workspace.resolve_path(path)?;
        if !self.workspace.can_access(&resolved, write)? {
            return Err(NodeError::Permission(format!(
                "Cannot access path: {:?}",
                resolved
            )));
        }
        Ok(resolved)
    }

    fn resolve_execution_dir(&self, dir: Option<&str>) -> NodeResult<PathBuf> {
        let resolved = if let Some(dir) = dir {
            self.workspace.resolve_path(dir)?
        } else {
            self.workspace.root().to_path_buf()
        };

        if !self.workspace.can_access(&resolved, false)? {
            return Err(NodeError::Permission(format!(
                "Working directory outside workspace: {:?}",
                resolved
            )));
        }

        Ok(resolved)
    }

    fn resolve_code_workdir(&self, cmd: &CodeCommand) -> NodeResult<PathBuf> {
        self.resolve_execution_dir(cmd.workdir.as_deref())
    }

    async fn ensure_internal_work_dir(&self) -> NodeResult<PathBuf> {
        let internal_dir = self.workspace.internal_path(&self.internal_work_dir);
        if !self.workspace.can_access(&internal_dir, true)? {
            return Err(NodeError::Permission(format!(
                "Internal work directory outside workspace: {:?}",
                internal_dir
            )));
        }

        tokio::fs::create_dir_all(&internal_dir)
            .await
            .map_err(|e| {
                NodeError::Execution(format!(
                    "Failed to create internal work directory {:?}: {}",
                    internal_dir, e
                ))
            })?;

        Ok(internal_dir)
    }

    async fn create_temp_code_file(&self, extension: &str, content: &str) -> NodeResult<PathBuf> {
        let internal_dir = self.ensure_internal_work_dir().await?;
        let file_path = internal_dir.join(format!(
            "code-{}.{}",
            uuid::Uuid::new_v4(),
            extension.trim_start_matches('.')
        ));

        tokio::fs::write(&file_path, content).await.map_err(|e| {
            NodeError::Execution(format!("Failed to write temp file {:?}: {}", file_path, e))
        })?;

        Ok(file_path)
    }

    async fn run_process(
        &self,
        mut command: TokioCommand,
        timeout_duration: Duration,
    ) -> NodeResult<CommandOutput> {
        let timeout_duration = if timeout_duration.is_zero() {
            self.default_timeout
        } else {
            timeout_duration
        };

        let output = match tokio::time::timeout(timeout_duration, command.output()).await {
            Ok(result) => result
                .map_err(|e| NodeError::Execution(format!("Failed to execute command: {}", e)))?,
            Err(_) => {
                return Err(NodeError::Timeout(format!(
                    "Command timed out after {:?}",
                    timeout_duration
                )))
            }
        };

        let stdout = self.limit_output(String::from_utf8_lossy(&output.stdout).to_string());
        let stderr = self.limit_output(String::from_utf8_lossy(&output.stderr).to_string());

        if output.status.success() {
            if !stderr.is_empty() {
                Ok(CommandOutput::text(format!(
                    "{}{}{}",
                    stdout,
                    if stdout.is_empty() { "" } else { "\n" },
                    stderr
                )))
            } else {
                Ok(CommandOutput::text(stdout))
            }
        } else {
            let message = if stderr.is_empty() {
                format!("Command failed with exit code {:?}", output.status.code())
            } else {
                format!(
                    "Command failed with exit code {:?}: {}",
                    output.status.code(),
                    stderr
                )
            };
            Err(NodeError::Execution(message))
        }
    }

    fn limit_output(&self, output: String) -> String {
        if output.len() <= self.max_output_size {
            return output;
        }

        let truncated = &output[..self.max_output_size];
        format!("{}\n... output truncated ...", truncated)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use uhorse_protocol::{CodeLanguage, SessionId, UserId};

    #[cfg(feature = "browser")]
    use uhorse_protocol::BrowserResult;

    fn create_context() -> TaskContext {
        TaskContext::new(
            UserId::from_string("test-user"),
            SessionId::from_string("test-session"),
            "test-channel",
        )
    }

    fn create_executor(temp: &TempDir) -> CommandExecutor {
        let workspace = Arc::new(Workspace::new(temp.path()).unwrap());
        let permission_manager = Arc::new(PermissionManager::new(workspace.clone(), true));
        CommandExecutor::new(workspace, permission_manager, ".uhorse")
    }

    #[tokio::test]
    async fn test_execute_shell_defaults_to_workspace_root() {
        let temp = TempDir::new().unwrap();
        let executor = create_executor(&temp);
        executor.permission_manager.load_default_rules().await;

        let result = executor
            .execute_shell(&ShellCommand::new("pwd"))
            .await
            .unwrap();

        match result {
            CommandOutput::Text { content } => {
                assert_eq!(
                    content.trim(),
                    temp.path().canonicalize().unwrap().to_string_lossy()
                );
            }
            other => panic!("unexpected output: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_shell_timeout_is_enforced() {
        let temp = TempDir::new().unwrap();
        let executor = create_executor(&temp);
        executor.permission_manager.load_default_rules().await;

        let command = ShellCommand::new("sh")
            .with_args(vec!["-c".to_string(), "sleep 2".to_string()])
            .with_timeout(Duration::from_millis(100));

        let error = executor.execute_shell(&command).await.unwrap_err();
        assert!(matches!(error, NodeError::Timeout(_)));
    }

    #[tokio::test]
    async fn test_python_temp_file_is_created_inside_workspace_internal_dir() {
        let temp = TempDir::new().unwrap();
        let executor = create_executor(&temp);
        executor.permission_manager.load_default_rules().await;

        let command = CodeCommand::new(
            CodeLanguage::Python,
            "import pathlib\nprint(pathlib.Path(__file__).resolve())",
        );

        let result = executor.execute_code(&command).await.unwrap();
        match result {
            CommandOutput::Text { content } => {
                let temp_path = content.trim();
                assert!(temp_path.contains(".uhorse"));
                assert!(temp_path.starts_with(
                    &temp
                        .path()
                        .canonicalize()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                ));
            }
            other => panic!("unexpected output: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_file_exists_respects_workspace_boundary() {
        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let executor = create_executor(&temp);
        executor.permission_manager.load_default_rules().await;

        let error = executor
            .file_exists(&outside.path().join("outside.txt").to_string_lossy())
            .await
            .unwrap_err();

        assert!(matches!(error, NodeError::Permission(_)));
    }

    #[tokio::test]
    async fn test_execute_wraps_permission_denial_as_failure_result() {
        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let executor = create_executor(&temp);
        executor.permission_manager.load_default_rules().await;

        let command = Command::Shell(
            ShellCommand::new("pwd").with_cwd(outside.path().to_string_lossy().to_string()),
        );
        let result = executor
            .execute(&TaskId::from_string("task-1"), &command, &create_context())
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_file_write_returns_structured_path_result() {
        let temp = TempDir::new().unwrap();
        let executor = create_executor(&temp);
        executor.permission_manager.load_default_rules().await;

        let result = executor
            .file_write("notes/todo.md", "hello", true)
            .await
            .unwrap();

        match result {
            CommandOutput::Json { content } => {
                assert_eq!(content["kind"], "file_operation");
                assert_eq!(content["action"], "write");
                assert_eq!(content["bytes_written"], 5);
                assert_eq!(
                    content["path"],
                    temp.path()
                        .join("notes/todo.md")
                        .canonicalize()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                );
            }
            other => panic!("unexpected output: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_file_append_returns_structured_path_result() {
        let temp = TempDir::new().unwrap();
        let executor = create_executor(&temp);
        executor.permission_manager.load_default_rules().await;

        let result = executor
            .file_append("notes/todo.md", "hello")
            .await
            .unwrap();

        match result {
            CommandOutput::Json { content } => {
                assert_eq!(content["kind"], "file_operation");
                assert_eq!(content["action"], "append");
                assert_eq!(content["bytes_appended"], 5);
                assert_eq!(
                    content["path"],
                    temp.path()
                        .join("notes/todo.md")
                        .canonicalize()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                );
            }
            other => panic!("unexpected output: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_file_write_creates_parent_directories_and_persists_content() {
        let temp = TempDir::new().unwrap();
        let executor = create_executor(&temp);
        executor.permission_manager.load_default_rules().await;

        executor
            .file_write("deep/nested/notes.txt", "workspace content", true)
            .await
            .unwrap();

        assert_eq!(
            tokio::fs::read_to_string(temp.path().join("deep/nested/notes.txt"))
                .await
                .unwrap(),
            "workspace content"
        );
    }

    #[tokio::test]
    async fn test_file_write_rejects_parent_escape_path() {
        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let executor = create_executor(&temp);
        executor.permission_manager.load_default_rules().await;
        let escaped_path = format!("../{}/escape.txt", outside.path().file_name().unwrap().to_string_lossy());

        let error = executor
            .file_write(&escaped_path, "blocked", true)
            .await
            .unwrap_err();

        assert!(matches!(error, NodeError::Permission(_)));
        assert!(!outside.path().join("escape.txt").exists());
    }

    #[tokio::test]
    async fn test_file_copy_returns_structured_paths() {
        let temp = TempDir::new().unwrap();
        let executor = create_executor(&temp);
        executor.permission_manager.load_default_rules().await;

        tokio::fs::write(temp.path().join("from.txt"), "copy me")
            .await
            .unwrap();
        let result = executor
            .file_copy("from.txt", "nested/to.txt", true)
            .await
            .unwrap();

        match result {
            CommandOutput::Json { content } => {
                assert_eq!(content["kind"], "file_operation");
                assert_eq!(content["action"], "copy");
                assert_eq!(content["bytes_copied"], 7);
                assert_eq!(
                    content["source_path"],
                    temp.path()
                        .join("from.txt")
                        .canonicalize()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                );
                assert_eq!(
                    content["destination_path"],
                    temp.path()
                        .join("nested/to.txt")
                        .canonicalize()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                );
            }
            other => panic!("unexpected output: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_file_move_returns_structured_paths() {
        let temp = TempDir::new().unwrap();
        let executor = create_executor(&temp);
        executor.permission_manager.load_default_rules().await;

        tokio::fs::write(temp.path().join("from.txt"), "move me")
            .await
            .unwrap();
        let source_path = temp
            .path()
            .join("from.txt")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let result = executor
            .file_move("from.txt", "nested/to.txt", true)
            .await
            .unwrap();

        match result {
            CommandOutput::Json { content } => {
                assert_eq!(content["kind"], "file_operation");
                assert_eq!(content["action"], "move");
                assert_eq!(content["source_path"], source_path);
                assert_eq!(
                    content["destination_path"],
                    temp.path()
                        .join("nested/to.txt")
                        .canonicalize()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                );
            }
            other => panic!("unexpected output: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_file_create_dir_returns_structured_path_result() {
        let temp = TempDir::new().unwrap();
        let executor = create_executor(&temp);
        executor.permission_manager.load_default_rules().await;

        let result = executor.file_create_dir("nested/dir", true).await.unwrap();

        match result {
            CommandOutput::Json { content } => {
                assert_eq!(content["kind"], "file_operation");
                assert_eq!(content["action"], "create_dir");
                assert_eq!(content["recursive"], true);
                assert_eq!(
                    content["path"],
                    temp.path()
                        .join("nested/dir")
                        .canonicalize()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                );
            }
            other => panic!("unexpected output: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_execute_browser_returns_feature_disabled_error_without_browser_feature() {
        let temp = TempDir::new().unwrap();
        let executor = create_executor(&temp);
        executor.permission_manager.load_default_rules().await;

        let result = executor
            .execute_unchecked(&Command::Browser(BrowserCommand::Navigate {
                url: "https://example.com".to_string(),
            }))
            .await;

        #[cfg(not(feature = "browser"))]
        {
            let error = result.unwrap_err();
            assert!(matches!(error, NodeError::Execution(_)));
            assert!(error
                .to_string()
                .contains("Browser support not enabled. Compile with 'browser' feature"));
        }

        #[cfg(feature = "browser")]
        {
            match result {
                Ok(result) => match result.output {
                    CommandOutput::Browser {
                        result: BrowserResult::Navigate { .. },
                    }
                    | CommandOutput::Browser {
                        result: BrowserResult::Error { .. },
                    } => {}
                    other => panic!("unexpected output: {:?}", other),
                },
                Err(error) => assert!(matches!(error, NodeError::Execution(_))),
            }
        }
    }
}
