//! 工作区版本管理
//!
//! 提供基于 Git 的版本管理能力，供 Node 客户端查看变更、创建检查点与安全恢复。

use crate::error::{NodeError, NodeResult};
use crate::workspace::Workspace;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

/// 工作区版本管理器
#[derive(Debug, Clone)]
pub struct VersionManager {
    workspace: Arc<Workspace>,
}

impl VersionManager {
    /// 创建新的版本管理器
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self { workspace }
    }

    /// 获取工作区当前状态
    pub fn status(&self) -> NodeResult<WorkspaceVersionStatus> {
        self.ensure_git_repo()?;
        let output = self.git(["status", "--short", "--branch"])?;
        WorkspaceVersionStatus::from_git_status(self.workspace.root(), &output)
    }

    /// 获取工作区 diff
    pub fn diff(&self, target: DiffTarget) -> NodeResult<WorkspaceDiff> {
        self.ensure_git_repo()?;
        let output = match &target {
            DiffTarget::WorkingTree => self.git(["diff", "--"]),
            DiffTarget::Staged => self.git(["diff", "--cached", "--"]),
            DiffTarget::RevisionRange { from, to } => {
                self.git(["diff", from.as_str(), to.as_str(), "--"])
            }
        }?;
        Ok(WorkspaceDiff {
            target,
            patch: output,
            generated_at: Utc::now(),
        })
    }

    /// 创建检查点提交
    pub fn create_checkpoint(
        &self,
        message: impl Into<String>,
        include_untracked: bool,
    ) -> NodeResult<CheckpointRecord> {
        self.ensure_git_repo()?;
        let message = message.into();

        if include_untracked {
            self.git(["add", "."])?;
        } else {
            self.git(["add", "-u"])?;
        }

        let status = self.git(["status", "--short"])?;
        if status.trim().is_empty() {
            return Err(NodeError::Execution(
                "No workspace changes available for checkpoint".to_string(),
            ));
        }

        self.git(["commit", "-m", &message])?;
        self.current_checkpoint()
    }

    /// 获取最近一次检查点
    pub fn current_checkpoint(&self) -> NodeResult<CheckpointRecord> {
        self.ensure_git_repo()?;
        let output = self.git(["log", "-1", "--pretty=format:%H%x1f%s%x1f%cI"])?;
        CheckpointRecord::from_log_line(&output)
    }

    /// 列出最近检查点
    pub fn list_checkpoints(&self, limit: usize) -> NodeResult<Vec<CheckpointRecord>> {
        self.ensure_git_repo()?;
        let limit_arg = limit.max(1).to_string();
        let output = self.git([
            "log",
            &format!("-{}", limit_arg),
            "--pretty=format:%H%x1f%s%x1f%cI",
        ])?;

        output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(CheckpointRecord::from_log_line)
            .collect()
    }

    /// 预览恢复目标与当前工作区差异
    pub fn preview_restore(&self, revision: &str) -> NodeResult<RestorePreview> {
        self.ensure_git_repo()?;
        let patch = self.git(["diff", revision, "--"])?;
        let status_before = self.status()?;
        Ok(RestorePreview {
            revision: revision.to_string(),
            patch,
            status_before,
            generated_at: Utc::now(),
        })
    }

    /// 安全恢复到指定 revision
    pub fn restore(&self, revision: &str) -> NodeResult<RestoreResult> {
        self.ensure_git_repo()?;
        let before = self.status()?;
        self.git(["reset", "--soft", revision])?;
        let after = self.status()?;
        Ok(RestoreResult {
            revision: revision.to_string(),
            restored_at: Utc::now(),
            status_before: before,
            status_after: after,
        })
    }

    fn ensure_git_repo(&self) -> NodeResult<()> {
        if self.workspace.is_git_repo() {
            Ok(())
        } else {
            Err(NodeError::Execution(format!(
                "Workspace is not a git repository: {}",
                self.workspace.root().display()
            )))
        }
    }

    fn git<I, S>(&self, args: I) -> NodeResult<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let collected: Vec<String> = args
            .into_iter()
            .map(|arg| arg.as_ref().to_string())
            .collect();
        let output = Command::new("git")
            .arg("-C")
            .arg(self.workspace.root())
            .args(&collected)
            .output()
            .map_err(|error| NodeError::Execution(format!("Failed to run git: {}", error)))?;

        if !output.status.success() {
            return Err(NodeError::Execution(format!(
                "git {} failed: {}",
                collected.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

/// 工作区版本状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceVersionStatus {
    /// 当前分支
    pub branch: String,
    /// 是否存在未提交变更
    pub dirty: bool,
    /// 文件变更列表
    pub entries: Vec<VersionStatusEntry>,
    /// 生成时间
    pub generated_at: DateTime<Utc>,
}

impl WorkspaceVersionStatus {
    fn from_git_status(root: &Path, output: &str) -> NodeResult<Self> {
        let mut branch = String::from("HEAD");
        let mut entries = Vec::new();

        for line in output.lines() {
            if let Some(rest) = line.strip_prefix("## ") {
                branch = rest.split("...").next().unwrap_or(rest).trim().to_string();
                continue;
            }

            if line.trim().is_empty() {
                continue;
            }

            entries.push(VersionStatusEntry::from_porcelain_line(root, line)?);
        }

        Ok(Self {
            branch,
            dirty: !entries.is_empty(),
            entries,
            generated_at: Utc::now(),
        })
    }
}

/// 单个文件状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionStatusEntry {
    /// 文件相对路径
    pub path: String,
    /// 索引状态
    pub staged_status: FileChangeKind,
    /// 工作区状态
    pub unstaged_status: FileChangeKind,
}

impl VersionStatusEntry {
    fn from_porcelain_line(root: &Path, line: &str) -> NodeResult<Self> {
        if line.len() < 4 {
            return Err(NodeError::Execution(format!(
                "Invalid git status line in {}: {}",
                root.display(),
                line
            )));
        }

        let staged = line.chars().next().unwrap_or(' ');
        let unstaged = line.chars().nth(1).unwrap_or(' ');
        let path = line[3..].trim().to_string();

        Ok(Self {
            path,
            staged_status: FileChangeKind::from_porcelain(staged),
            unstaged_status: FileChangeKind::from_porcelain(unstaged),
        })
    }
}

/// 文件变更类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    /// 无变更
    Unmodified,
    /// 新增
    Added,
    /// 修改
    Modified,
    /// 删除
    Deleted,
    /// 重命名
    Renamed,
    /// 复制
    Copied,
    /// 未跟踪
    Untracked,
    /// 冲突
    Unmerged,
    /// 未知
    Unknown,
}

impl FileChangeKind {
    fn from_porcelain(value: char) -> Self {
        match value {
            ' ' => Self::Unmodified,
            'A' => Self::Added,
            'M' => Self::Modified,
            'D' => Self::Deleted,
            'R' => Self::Renamed,
            'C' => Self::Copied,
            '?' => Self::Untracked,
            'U' => Self::Unmerged,
            _ => Self::Unknown,
        }
    }
}

/// diff 目标
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffTarget {
    /// 工作区与索引之间
    WorkingTree,
    /// 索引与 HEAD 之间
    Staged,
    /// 两个 revision 之间
    RevisionRange { from: String, to: String },
}

/// 工作区 diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDiff {
    /// diff 目标
    pub target: DiffTarget,
    /// patch 内容
    pub patch: String,
    /// 生成时间
    pub generated_at: DateTime<Utc>,
}

/// 检查点记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointRecord {
    /// 提交哈希
    pub revision: String,
    /// 提交消息
    pub message: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

impl CheckpointRecord {
    fn from_log_line(line: &str) -> NodeResult<Self> {
        let mut parts = line.split('\u{001f}');
        let revision = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| NodeError::Execution("Missing checkpoint revision".to_string()))?;
        let message = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| NodeError::Execution("Missing checkpoint message".to_string()))?;
        let created_at = parts
            .next()
            .ok_or_else(|| NodeError::Execution("Missing checkpoint timestamp".to_string()))?;

        Ok(Self {
            revision: revision.to_string(),
            message: message.to_string(),
            created_at: DateTime::parse_from_rfc3339(created_at)
                .map_err(|error| {
                    NodeError::Execution(format!("Invalid checkpoint timestamp: {}", error))
                })?
                .with_timezone(&Utc),
        })
    }
}

/// 恢复预览
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePreview {
    /// 目标 revision
    pub revision: String,
    /// patch 内容
    pub patch: String,
    /// 恢复前状态
    pub status_before: WorkspaceVersionStatus,
    /// 生成时间
    pub generated_at: DateTime<Utc>,
}

/// 恢复结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    /// 目标 revision
    pub revision: String,
    /// 恢复时间
    pub restored_at: DateTime<Utc>,
    /// 恢复前状态
    pub status_before: WorkspaceVersionStatus,
    /// 恢复后状态
    pub status_after: WorkspaceVersionStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_git_repo() -> (TempDir, Arc<Workspace>, VersionManager) {
        let temp = TempDir::new().unwrap();
        run_git(temp.path(), ["init"]).unwrap();
        run_git(temp.path(), ["config", "user.name", "uHorse Test"]).unwrap();
        run_git(temp.path(), ["config", "user.email", "test@uhorse.local"]).unwrap();
        fs::write(temp.path().join("README.md"), "hello\n").unwrap();
        run_git(temp.path(), ["add", "."]).unwrap();
        run_git(temp.path(), ["commit", "-m", "initial"]).unwrap();

        let workspace = Arc::new(Workspace::new(temp.path()).unwrap());
        let manager = VersionManager::new(workspace.clone());
        (temp, workspace, manager)
    }

    fn run_git<I, S>(root: &Path, args: I) -> NodeResult<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(
                args.into_iter()
                    .map(|arg| arg.as_ref().to_string())
                    .collect::<Vec<_>>(),
            )
            .output()
            .unwrap();
        if output.status.success() {
            Ok(())
        } else {
            Err(NodeError::Execution(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ))
        }
    }

    #[test]
    fn test_status_reports_dirty_workspace() {
        let (temp, _workspace, manager) = init_git_repo();
        fs::write(temp.path().join("README.md"), "hello\nworld\n").unwrap();

        let status = manager.status().unwrap();
        assert!(status.dirty);
        assert!(matches!(status.branch.as_str(), "master" | "main"));
        assert_eq!(status.entries.len(), 1);
        assert_eq!(status.entries[0].path, "README.md");
        assert_eq!(status.entries[0].unstaged_status, FileChangeKind::Modified);
    }

    #[test]
    fn test_list_checkpoints_returns_latest_commit() {
        let (_temp, _workspace, manager) = init_git_repo();
        let checkpoints = manager.list_checkpoints(5).unwrap();

        assert_eq!(checkpoints.len(), 1);
        assert_eq!(checkpoints[0].message, "initial");
        assert!(!checkpoints[0].revision.is_empty());
    }

    #[test]
    fn test_create_checkpoint_commits_changes() {
        let (temp, _workspace, manager) = init_git_repo();
        fs::write(temp.path().join("notes.txt"), "draft\n").unwrap();

        let checkpoint = manager
            .create_checkpoint("checkpoint: 保存 notes", true)
            .unwrap();
        assert_eq!(checkpoint.message, "checkpoint: 保存 notes");

        let status = manager.status().unwrap();
        assert!(!status.dirty);
    }

    #[test]
    fn test_preview_restore_generates_patch() {
        let (temp, _workspace, manager) = init_git_repo();
        fs::write(temp.path().join("README.md"), "changed\n").unwrap();
        let head = manager.current_checkpoint().unwrap();

        let preview = manager.preview_restore(&head.revision).unwrap();
        assert_eq!(preview.revision, head.revision);
        assert!(preview.patch.contains("diff --git"));
    }

    #[test]
    fn test_restore_uses_soft_reset() {
        let (temp, _workspace, manager) = init_git_repo();
        fs::write(temp.path().join("notes.txt"), "draft\n").unwrap();
        let initial = manager.current_checkpoint().unwrap();
        manager
            .create_checkpoint("checkpoint: 保存 notes", true)
            .unwrap();

        let restored = manager.restore(&initial.revision).unwrap();
        assert_eq!(restored.revision, initial.revision);
        assert!(restored.status_after.dirty);
    }
}
