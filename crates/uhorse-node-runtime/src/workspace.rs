//! 工作空间管理
//!
//! 定义用户授权的工作目录范围

use crate::error::{NodeError, NodeResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use tracing::{debug, info};

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
        let canonical_path = Self::canonicalize_root(path.as_ref())?;

        let config = WorkspaceConfig {
            name: canonical_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace")
                .to_string(),
            root_path: canonical_path,
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
    pub fn with_config(mut config: WorkspaceConfig) -> NodeResult<Self> {
        config.root_path = Self::canonicalize_root(&config.root_path)?;

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

    /// 解析工作空间内路径
    pub fn resolve_path<P: AsRef<Path>>(&self, path: P) -> NodeResult<PathBuf> {
        let path = path.as_ref();
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.config.root_path.join(path)
        };

        Self::normalize_existing_aware(&candidate)
    }

    /// 获取内部目录路径
    pub fn internal_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.config.root_path.join(path)
    }

    /// 检查是否为 git 工作树
    pub fn is_git_repo(&self) -> bool {
        matches!(
            Command::new("git")
                .arg("-C")
                .arg(&self.config.root_path)
                .arg("rev-parse")
                .arg("--is-inside-work-tree")
                .output(),
            Ok(output)
                if output.status.success()
                    && String::from_utf8_lossy(&output.stdout).trim() == "true"
        )
    }

    /// 检查路径是否在工作空间内
    pub fn contains<P: AsRef<Path>>(&self, path: P) -> bool {
        self.resolve_path(path)
            .map(|resolved| resolved.starts_with(&self.config.root_path))
            .unwrap_or(false)
    }

    /// 检查路径是否可访问
    pub fn can_access<P: AsRef<Path>>(&self, path: P, write: bool) -> NodeResult<bool> {
        let resolved = self.resolve_path(path)?;

        // 1. 检查是否在工作空间内
        if !resolved.starts_with(&self.config.root_path) {
            debug!("Path not in workspace: {:?}", resolved);
            return Ok(false);
        }

        // 2. 检查只读模式
        if write && self.config.read_only {
            debug!("Workspace is read-only");
            return Ok(false);
        }

        let relative = resolved
            .strip_prefix(&self.config.root_path)
            .unwrap_or_else(|_| Path::new(""));
        let relative_str = if relative.as_os_str().is_empty() {
            ".".to_string()
        } else {
            relative.to_string_lossy().replace('\\', "/")
        };

        // 3. 检查禁止模式
        for pattern in &self.config.denied_patterns {
            if glob_match::glob_match(pattern, &relative_str) {
                debug!("Path matches denied pattern: {}", pattern);
                return Ok(false);
            }
        }

        // 4. 工作空间根目录始终允许访问
        if relative_str == "." {
            return Ok(true);
        }

        // 5. 检查允许模式
        let allowed = self
            .config
            .allowed_patterns
            .iter()
            .any(|pattern| glob_match::glob_match(pattern, &relative_str));

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
        let resolved = self.resolve_path(path)?;

        // 检查访问权限
        if !self.can_access(&resolved, false)? {
            return Err(NodeError::Permission(format!(
                "Cannot access path: {:?}",
                resolved
            )));
        }

        let metadata = std::fs::metadata(&resolved)
            .map_err(|e| NodeError::Workspace(format!("Failed to get metadata: {}", e)))?;

        let name = resolved
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(FileInfo {
            path: resolved.to_string_lossy().to_string(),
            name,
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            modified: metadata.modified().ok().map(|t| t.into()),
            created: metadata.created().ok().map(|t| t.into()),
            mode: None,
        })
    }

    /// 列出目录内容
    pub fn list_dir<P: AsRef<Path>>(&self, path: P) -> NodeResult<Vec<FileInfo>> {
        let resolved = self.resolve_path(path)?;

        // 检查访问权限
        if !self.can_access(&resolved, false)? {
            return Err(NodeError::Permission(format!(
                "Cannot access path: {:?}",
                resolved
            )));
        }

        let mut entries = Vec::new();
        let dir = std::fs::read_dir(&resolved)
            .map_err(|e| NodeError::Workspace(format!("Failed to read directory: {}", e)))?;

        for entry in dir {
            let entry =
                entry.map_err(|e| NodeError::Workspace(format!("Failed to read entry: {}", e)))?;
            let entry_path = entry.path();

            if !self.can_access(&entry_path, false)? {
                continue;
            }

            if let Ok(info) = self.get_file_info(entry_path) {
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
        let entries = std::fs::read_dir(dir).map_err(|e| {
            NodeError::Workspace(format!("Failed to read directory {:?}: {}", dir, e))
        })?;

        for entry in entries {
            let entry =
                entry.map_err(|e| NodeError::Workspace(format!("Failed to read entry: {}", e)))?;
            let path = entry.path();

            if !self.can_access(&path, false)? {
                continue;
            }

            self.file_index.insert(path.clone());
            *count += 1;

            if path.is_dir() {
                self.walk_directory(&path, count)?;
            }
        }

        Ok(())
    }

    /// 获取配置
    pub fn config(&self) -> &WorkspaceConfig {
        &self.config
    }

    /// 更新配置
    pub fn update_config(&mut self, mut config: WorkspaceConfig) -> NodeResult<()> {
        config.root_path = Self::canonicalize_root(&config.root_path)?;

        self.config = config;
        self.file_index.clear();
        self.last_index_update = None;

        Ok(())
    }

    /// 转换为协议类型
    pub fn to_protocol(&self) -> uhorse_protocol::WorkspaceInfo {
        uhorse_protocol::WorkspaceInfo {
            workspace_id: None,
            name: self.config.name.clone(),
            path: self.config.root_path.to_string_lossy().to_string(),
            read_only: self.config.read_only,
            allowed_patterns: self.config.allowed_patterns.clone(),
            denied_patterns: self.config.denied_patterns.clone(),
        }
    }

    fn canonicalize_root(path: &Path) -> NodeResult<PathBuf> {
        if !path.exists() {
            return Err(NodeError::Workspace(format!(
                "Path does not exist: {:?}",
                path
            )));
        }

        if !path.is_dir() {
            return Err(NodeError::Workspace(format!(
                "Path is not a directory: {:?}",
                path
            )));
        }

        path.canonicalize()
            .map_err(|e| NodeError::Workspace(format!("Failed to canonicalize path: {}", e)))
    }

    fn normalize_existing_aware(path: &Path) -> NodeResult<PathBuf> {
        let mut existing_ancestor = Some(path);
        let ancestor = loop {
            match existing_ancestor {
                Some(candidate) if candidate.exists() => break candidate.to_path_buf(),
                Some(candidate) => existing_ancestor = candidate.parent(),
                None => {
                    return Err(NodeError::Workspace(format!(
                        "Failed to resolve path: {:?}",
                        path
                    )))
                }
            }
        };

        let mut normalized = ancestor.canonicalize().map_err(|e| {
            NodeError::Workspace(format!(
                "Failed to canonicalize ancestor {:?}: {}",
                ancestor, e
            ))
        })?;

        let remainder = path.strip_prefix(&ancestor).map_err(|_| {
            NodeError::Workspace(format!("Failed to resolve relative path for {:?}", path))
        })?;

        for component in remainder.components() {
            match component {
                Component::CurDir => {}
                Component::Normal(part) => normalized.push(part),
                Component::ParentDir => {
                    normalized.pop();
                }
                Component::RootDir | Component::Prefix(_) => {}
            }
        }

        Ok(normalized)
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

        let canonical_temp = temp.path().canonicalize().unwrap();
        assert!(workspace.contains(temp.path()));
        assert_eq!(workspace.root(), canonical_temp);
    }

    #[test]
    fn test_workspace_can_access_existing_file() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("test.txt");
        std::fs::write(&file, "test").unwrap();

        let workspace = Workspace::new(temp.path()).unwrap();

        assert!(workspace.can_access(&file, false).unwrap());
        assert!(workspace.can_access(&file, true).unwrap());
    }

    #[test]
    fn test_workspace_contains_new_file_and_directory() {
        let temp = TempDir::new().unwrap();
        let workspace = Workspace::new(temp.path()).unwrap();

        let new_dir = temp.path().join("new-dir");
        let new_file = new_dir.join("test.txt");

        assert!(workspace.contains(&new_dir));
        assert!(workspace.contains(&new_file));
        assert!(workspace.can_access(&new_dir, true).unwrap());
        assert!(workspace.can_access(&new_file, true).unwrap());
    }

    #[test]
    fn test_workspace_rejects_parent_escape() {
        let temp = TempDir::new().unwrap();
        let workspace = Workspace::new(temp.path()).unwrap();
        let escaped = temp.path().join("..").join("escaped.txt");

        assert!(!workspace.contains(&escaped));
        assert!(!workspace.can_access(&escaped, true).unwrap());
    }

    #[test]
    fn test_workspace_rejects_absolute_outside_path() {
        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let workspace = Workspace::new(temp.path()).unwrap();
        let outside_file = outside.path().join("outside.txt");

        assert!(!workspace.contains(&outside_file));
        assert!(!workspace.can_access(&outside_file, false).unwrap());
    }

    #[test]
    fn test_workspace_resolves_relative_paths_against_root() {
        let temp = TempDir::new().unwrap();
        let workspace = Workspace::new(temp.path()).unwrap();
        let resolved = workspace.resolve_path("nested/file.txt").unwrap();

        assert_eq!(resolved, workspace.root().join("nested/file.txt"));
    }

    #[test]
    fn test_workspace_resolve_path_normalizes_parent_escape_outside_root() {
        let temp = TempDir::new().unwrap();
        let workspace = Workspace::new(temp.path()).unwrap();
        let resolved = workspace.resolve_path("../outside.txt").unwrap();

        assert_eq!(
            resolved,
            workspace.root().parent().unwrap().join("outside.txt")
        );
        assert!(!workspace.contains("../outside.txt"));
        assert!(!workspace.can_access("../outside.txt", true).unwrap());
    }
}
