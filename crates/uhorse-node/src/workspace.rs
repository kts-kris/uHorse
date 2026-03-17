//! 工作空间管理
//!
//! 定义用户授权的工作目录范围

use crate::error::{NodeError, NodeResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use tracing::{debug, info, warn};

/// 工作空间配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// 工作空间名称
    pub name: String,

    /// 根目录路径
    pub root_path: PathBuf,

    /// 是否只读
    pub read_only: bool,

    /// 允许的文件模式（glob）
    #[serde(default)]
    pub allowed_patterns: Vec<String>,

    /// 禁止的文件模式（glob）
    #[serde(default)]
    pub denied_patterns: Vec<String>,

    /// 最大文件大小（字节）
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// 是否允许符号链接
    #[serde(default)]
    pub allow_symlinks: bool,

    /// 是否允许执行脚本
    #[serde(default)]
    pub allow_execution: bool,
}

fn default_max_file_size() -> u64 {
    100 * 1024 * 1024 // 100 MB
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            root_path: PathBuf::from("."),
            read_only: false,
            allowed_patterns: vec!["**/*".to_string()],
            denied_patterns: vec![],
            max_file_size: default_max_file_size(),
            allow_symlinks: false,
            allow_execution: false,
        }
    }
}

/// 工作空间
#[derive(Debug)]
pub struct Workspace {
    /// 配置
    config: WorkspaceConfig,

    /// 文件索引（用于快速查找）
    file_index: HashSet<PathBuf>,

    /// 最后索引更新时间
    last_index_update: Option<DateTime<Utc>>,

    /// 索引是否启用
    index_enabled: bool,
}

impl Workspace {
    /// 创建新的工作空间
    pub fn new<P: AsRef<Path>>(path: P) -> NodeResult<Self> {
        let path = path.as_ref().to_path_buf();

        // 检查路径是否存在
        if !path.exists() {
            return Err(NodeError::Workspace(format!(
                "Path does not exist: {:?}",
                path
            )));
        }

        // 检查是否为目录
        if !path.is_dir() {
            return Err(NodeError::Workspace(format!(
                "Path is not a directory: {:?}",
                path
            )));
        }

        let config = WorkspaceConfig {
            name: path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace")
                .to_string(),
            root_path: path,
            ..Default::default()
        };

        Ok(Self {
            config,
            file_index: HashSet::new(),
            last_index_update: None,
            index_enabled: true,
        })
    }

    /// 使用配置创建工作空间
    pub fn with_config(config: WorkspaceConfig) -> NodeResult<Self> {
        // 检查路径是否存在
        if !config.root_path.exists() {
            return Err(NodeError::Workspace(format!(
                "Path does not exist: {:?}",
                config.root_path
            )));
        }

        Ok(Self {
            config,
            file_index: HashSet::new(),
            last_index_update: None,
            index_enabled: true,
        })
    }

    /// 获取工作空间名称
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// 获取根目录
    pub fn root(&self) -> &Path {
        &self.config.root_path
    }

    /// 检查路径是否在工作空间内
    pub fn contains<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();

        // 规范化路径
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };

        // 检查是否在根目录下
        canonical.starts_with(&self.config.root_path)
    }

    /// 检查路径是否可访问
    pub fn can_access<P: AsRef<Path>>(&self, path: P, write: bool) -> NodeResult<bool> {
        let path = path.as_ref();

        // 1. 检查是否在工作空间内
        if !self.contains(path) {
            debug!("Path not in workspace: {:?}", path);
            return Ok(false);
        }

        // 2. 检查只读模式
        if write && self.config.read_only {
            debug!("Workspace is read-only");
            return Ok(false);
        }

        // 3. 检查禁止模式
        let relative = path.strip_prefix(&self.config.root_path).unwrap_or(path);
        let relative_str = relative.to_string_lossy();

        for pattern in &self.config.denied_patterns {
            if glob_match::glob_match(pattern, &relative_str) {
                debug!("Path matches denied pattern: {}", pattern);
                return Ok(false);
            }
        }

        // 4. 检查允许模式
        let mut allowed = false;
        for pattern in &self.config.allowed_patterns {
            if glob_match::glob_match(pattern, &relative_str) {
                allowed = true;
                break;
            }
        }

        if !allowed {
            debug!("Path does not match any allowed pattern");
            return Ok(false);
        }

        Ok(true)
    }

    /// 检查路径是否可执行
    pub fn can_execute<P: AsRef<Path>>(&self, path: P) -> bool {
        if !self.config.allow_execution {
            return false;
        }

        self.can_access(path, false).unwrap_or(false)
    }

    /// 获取文件信息
    pub fn get_file_info<P: AsRef<Path>>(&self, path: P) -> NodeResult<FileInfo> {
        let path = path.as_ref();

        // 检查访问权限
        if !self.can_access(path, false)? {
            return Err(NodeError::Permission(format!(
                "Cannot access path: {:?}",
                path
            )));
        }

        let metadata = std::fs::metadata(path)
            .map_err(|e| NodeError::Workspace(format!("Failed to get metadata: {}", e)))?;

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(FileInfo {
            path: path.to_string_lossy().to_string(),
            name,
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            modified: metadata.modified().ok().map(|t| t.into()),
            created: metadata.created().ok().map(|t| t.into()),
            mode: None, // Unix mode not available on all platforms
        })
    }

    /// 列出目录内容
    pub fn list_dir<P: AsRef<Path>>(&self, path: P) -> NodeResult<Vec<FileInfo>> {
        let path = path.as_ref();

        // 检查访问权限
        if !self.can_access(path, false)? {
            return Err(NodeError::Permission(format!(
                "Cannot access path: {:?}",
                path
            )));
        }

        let mut entries = Vec::new();
        let dir = std::fs::read_dir(path)
            .map_err(|e| NodeError::Workspace(format!("Failed to read directory: {}", e)))?;

        for entry in dir {
            let entry = entry.map_err(|e| NodeError::Workspace(format!("Failed to read entry: {}", e)))?;

            if let Ok(info) = self.get_file_info(entry.path()) {
                entries.push(info);
            }
        }

        Ok(entries)
    }

    /// 刷新文件索引
    pub fn refresh_index(&mut self) -> NodeResult<usize> {
        if !self.index_enabled {
            return Ok(0);
        }

        info!("Refreshing file index for workspace: {}", self.config.name);

        let mut count = 0;
        self.file_index.clear();

        self.walk_directory(&self.config.root_path.clone(), &mut count)?;

        self.last_index_update = Some(Utc::now());
        info!("Indexed {} files", count);

        Ok(count)
    }

    /// 递归遍历目录
    fn walk_directory(&mut self, dir: &Path, count: &mut usize) -> NodeResult<()> {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| NodeError::Workspace(format!("Failed to read directory {:?}: {}", dir, e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| NodeError::Workspace(format!("Failed to read entry: {}", e)))?;
            let path = entry.path();

            // 检查是否可访问
            if self.can_access(&path, false)? {
                self.file_index.insert(path.clone());
                *count += 1;

                // 如果是目录，递归
                if path.is_dir() {
                    self.walk_directory(&path, count)?;
                }
            }
        }

        Ok(())
    }

    /// 获取配置
    pub fn config(&self) -> &WorkspaceConfig {
        &self.config
    }

    /// 更新配置
    pub fn update_config(&mut self, config: WorkspaceConfig) -> NodeResult<()> {
        // 检查新路径是否存在
        if !config.root_path.exists() {
            return Err(NodeError::Workspace(format!(
                "Path does not exist: {:?}",
                config.root_path
            )));
        }

        self.config = config;
        self.file_index.clear();
        self.last_index_update = None;

        Ok(())
    }

    /// 转换为协议类型
    pub fn to_protocol(&self) -> uhorse_protocol::WorkspaceInfo {
        uhorse_protocol::WorkspaceInfo {
            name: self.config.name.clone(),
            path: self.config.root_path.to_string_lossy().to_string(),
            read_only: self.config.read_only,
            allowed_patterns: self.config.allowed_patterns.clone(),
            denied_patterns: self.config.denied_patterns.clone(),
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

    /// 权限模式
    pub mode: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_workspace_new() {
        let temp = TempDir::new().unwrap();
        let workspace = Workspace::new(temp.path()).unwrap();

        assert!(workspace.contains(temp.path()));
        assert_eq!(workspace.root(), temp.path());
    }

    #[test]
    fn test_workspace_can_access() {
        let temp = TempDir::new().unwrap();

        // 创建一个测试文件
        let file = temp.path().join("test.txt");
        std::fs::write(&file, "test").unwrap();

        let workspace = Workspace::new(temp.path()).unwrap();

        assert!(workspace.can_access(&file, false).unwrap());
        assert!(workspace.can_access(&file, true).unwrap());
    }
}
