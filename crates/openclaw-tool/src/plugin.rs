//! # 插件运行时
//!
//! 支持外部进程插件的执行和沙箱隔离。

use openclaw_core::{Plugin, PluginError, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::collections::HashMap;
use tokio::process::{Command as TokioCommand, Child};
use tokio::io::{AsyncBufReadExt, BufReader, AsyncWriteExt, AsyncReadExt};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info, warn, error};

/// 插件配置
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// 插件名称
    pub name: String,
    /// 插件可执行文件路径
    pub executable: PathBuf,
    /// 工作目录
    pub working_dir: Option<PathBuf>,
    /// 环境变量
    pub env_vars: HashMap<String, String>,
    /// 沙箱配置
    pub sandbox: SandboxConfig,
}

/// 沙箱配置
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// 是否启用沙箱
    pub enabled: bool,
    /// 内存限制 (MB)
    pub memory_limit_mb: Option<usize>,
    /// CPU 限制 (百分比)
    pub cpu_limit_percent: Option<u8>,
    /// 超时时间 (秒)
    pub timeout_seconds: Option<u64>,
    /// 允许的网络访问
    pub allow_network: bool,
    /// 允许的文件访问路径
    pub allowed_paths: Vec<PathBuf>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            memory_limit_mb: Some(512),
            cpu_limit_percent: Some(50),
            timeout_seconds: Some(30),
            allow_network: false,
            allowed_paths: vec![],
        }
    }
}

/// 进程插件实现
#[derive(Debug)]
pub struct ProcessPlugin {
    config: PluginConfig,
    child: Arc<RwLock<Option<Child>>>,
}

impl ProcessPlugin {
    /// 创建新的进程插件
    pub fn new(config: PluginConfig) -> Self {
        Self {
            config,
            child: Arc::new(RwLock::new(None)),
        }
    }

    /// 从配置文件创建插件
    pub fn from_config_file(path: PathBuf) -> Result<Self> {
        // TODO: 实现从 JSON/TOML 文件加载配置
        Err(openclaw_core::OpenClawError::NotImplemented("Config file loading not implemented".to_string()))
    }
}

#[async_trait::async_trait]
impl Plugin for ProcessPlugin {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn initialize(&mut self) -> Result<(), PluginError> {
        info!("Initializing plugin: {}", self.config.name);

        let mut cmd = TokioCommand::new(&self.config.executable);

        // 设置工作目录
        if let Some(ref dir) = self.config.working_dir {
            cmd.current_dir(dir);
        }

        // 设置环境变量
        for (key, value) in &self.config.env_vars {
            cmd.env(key, value);
        }

        // 设置标准输入输出
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // 启动进程
        let child = cmd.spawn()
            .map_err(|e| PluginError::InitFailed(format!("Failed to spawn plugin process: {}", e)))?;

        // 存储子进程
        *self.child.write().await = Some(child);

        debug!("Plugin {} initialized successfully", self.config.name);
        Ok(())
    }

    async fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, PluginError> {
        debug!("Calling plugin method: {} with params: {:?}", method, params);

        // 构建请求
        let request = PluginRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: method.to_string(),
            params,
        };

        let request_json = serde_json::to_string(&request)
            .map_err(|e| PluginError::InvalidResponse(format!("Failed to serialize request: {}", e)))?;

        // 为每次调用创建新进程（简化实现，生产环境应使用进程池）
        let mut cmd = TokioCommand::new(&self.config.executable);

        if let Some(ref dir) = self.config.working_dir {
            cmd.current_dir(dir);
        }

        for (key, value) in &self.config.env_vars {
            cmd.env(key, value);
        }

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        cmd.arg("--method");
        cmd.arg(method);
        cmd.arg("--request");
        cmd.arg(&request_json);

        let output = cmd.output()
            .await
            .map_err(|e| PluginError::InvalidResponse(format!("Failed to execute plugin: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PluginError::InvalidResponse(format!("Plugin execution failed: {}", stderr)));
        }

        let response_str = String::from_utf8_lossy(&output.stdout);

        // 解析响应
        let response: PluginResponse = serde_json::from_str(&response_str)
            .map_err(|e| PluginError::InvalidResponse(format!("Failed to parse plugin response: {}", e)))?;

        if let Some(error) = response.error {
            Err(PluginError::InvalidResponse(error.message))
        } else {
            response.result.ok_or_else(|| PluginError::InvalidResponse("Empty result".to_string()))
        }
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        info!("Shutting down plugin: {}", self.config.name);

        let mut child_guard = self.child.write().await;
        if let Some(mut child) = child_guard.take() {
            // 尝试优雅关闭
            let _ = child.kill().await;
            let _ = child.wait().await;
        }

        debug!("Plugin {} shut down successfully", self.config.name);
        Ok(())
    }

    async fn health_check(&self) -> Result<(), PluginError> {
        let child_guard = self.child.read().await;
        if child_guard.as_ref().is_some() {
            // 尝试调用 ping 方法
            self.call("ping", serde_json::json!({})).await?;
            Ok(())
        } else {
            Err(PluginError::Crashed)
        }
    }
}

/// 插件请求格式
#[derive(Debug, Serialize)]
struct PluginRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: serde_json::Value,
}

/// 插件响应格式
#[derive(Debug, Deserialize)]
struct PluginResponse {
    jsonrpc: Option<String>,
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<PluginErrorDetail>,
}

#[derive(Debug, Deserialize)]
struct PluginErrorDetail {
    code: i32,
    message: String,
}

/// 插件运行时管理器
#[derive(Debug)]
pub struct PluginRuntime {
    plugins: Arc<RwLock<HashMap<String, Arc<RwLock<ProcessPlugin>>>>>,
    sandbox_config: SandboxConfig,
}

impl PluginRuntime {
    /// 创建新的插件运行时
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            sandbox_config: SandboxConfig::default(),
        }
    }

    /// 设置沙箱配置
    pub fn with_sandbox_config(mut self, config: SandboxConfig) -> Self {
        self.sandbox_config = config;
        self
    }

    /// 注册插件
    pub async fn register_plugin(&self, config: PluginConfig) -> Result<()> {
        let mut plugins = self.plugins.write().await;
        let plugin = ProcessPlugin::new(config.clone());
        plugins.insert(config.name.clone(), Arc::new(RwLock::new(plugin)));
        info!("Registered plugin: {}", config.name);
        Ok(())
    }

    /// 初始化所有插件
    pub async fn initialize_all(&self) -> Result<()> {
        let plugins = self.plugins.read().await;
        for (name, plugin) in plugins.iter() {
            debug!("Initializing plugin: {}", name);
            let mut plugin = plugin.write().await;
            plugin.initialize().await
                .map_err(|e| openclaw_core::OpenClawError::PluginError(e))?;
        }
        Ok(())
    }

    /// 调用插件方法
    pub async fn call(&self, plugin_name: &str, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let plugins = self.plugins.read().await;
        let plugin = plugins.get(plugin_name)
            .ok_or_else(|| PluginError::NotFound(plugin_name.to_string()))?;

        let plugin = plugin.read().await;
        plugin.call(method, params).await
            .map_err(|e| openclaw_core::OpenClawError::PluginError(e))
    }

    /// 关闭所有插件
    pub async fn shutdown_all(&self) -> Result<()> {
        let plugins = self.plugins.read().await;
        for (name, plugin) in plugins.iter() {
            debug!("Shutting down plugin: {}", name);
            let mut plugin = plugin.write().await;
            let _ = plugin.shutdown().await;
        }
        Ok(())
    }

    /// 健康检查所有插件
    pub async fn health_check_all(&self) -> HashMap<String, Result<(), PluginError>> {
        let plugins = self.plugins.read().await;
        let mut results = HashMap::new();

        for (name, plugin) in plugins.iter() {
            let plugin = plugin.read().await;
            results.insert(name.clone(), plugin.health_check().await);
        }

        results
    }
}

impl Default for PluginRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// 沙箱隔离层
#[derive(Debug)]
pub struct PluginSandbox {
    config: SandboxConfig,
}

impl PluginSandbox {
    /// 创建新的沙箱
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    /// 应用沙箱限制到命令
    pub fn apply_to_command(&self, cmd: &mut TokioCommand) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // 设置环境变量限制
        cmd.env("OPENCLAW_SANDBOX", "1");

        // 在 Linux 上，可以使用 prlimit 或 cgroup
        #[cfg(target_os = "linux")]
        {
            // TODO: 实现 cgroup 限制
            debug!("Linux sandbox limits would be applied here");
        }

        // 在 macOS 上，可以使用 sandbox_init
        #[cfg(target_os = "macos")]
        {
            debug!("macOS sandbox would be applied here");
        }

        Ok(())
    }

    /// 验证路径访问权限
    pub fn validate_path_access(&self, path: &PathBuf) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // 检查路径是否在允许列表中
        for allowed in &self.config.allowed_paths {
            if path.starts_with(allowed) {
                return Ok(());
            }
        }

        Err(openclaw_core::OpenClawError::InternalError(format!(
            "Path access denied: {:?}",
            path
        )))
    }

    /// 验证网络访问权限
    pub fn validate_network_access(&self) -> Result<()> {
        if !self.config.enabled || self.config.allow_network {
            return Ok(());
        }

        Err(openclaw_core::OpenClawError::InternalError(
            "Network access denied by sandbox policy".to_string()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_runtime_create() {
        let runtime = PluginRuntime::new();
        assert_eq!(runtime.plugins.read().await.len(), 0);
    }

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert!(config.enabled);
        assert_eq!(config.memory_limit_mb, Some(512));
        assert!(!config.allow_network);
    }
}
