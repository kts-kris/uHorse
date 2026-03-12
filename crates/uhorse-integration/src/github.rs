//! GitHub Integration
//!
//! GitHub Issue/PR 管理集成

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;

/// GitHub 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConfig {
    /// API Token (Personal Access Token)
    pub api_token: String,
    /// 默认仓库所有者
    pub default_owner: Option<String>,
    /// 默认仓库名
    pub default_repo: Option<String>,
    /// API 基础 URL (默认: https://api.github.com)
    #[serde(default = "default_api_url")]
    pub api_url: String,
}

fn default_api_url() -> String {
    "https://api.github.com".to_string()
}

impl GitHubConfig {
    /// 创建新配置
    pub fn new(api_token: impl Into<String>) -> Self {
        Self {
            api_token: api_token.into(),
            default_owner: None,
            default_repo: None,
            api_url: default_api_url(),
        }
    }

    /// 设置默认仓库
    pub fn with_default_repo(mut self, owner: impl Into<String>, repo: impl Into<String>) -> Self {
        self.default_owner = Some(owner.into());
        self.default_repo = Some(repo.into());
        self
    }

    /// 设置 API URL (用于 GitHub Enterprise)
    pub fn with_api_url(mut self, url: impl Into<String>) -> Self {
        self.api_url = url.into();
        self
    }
}

/// GitHub Issue 状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IssueState {
    /// 开放
    Open,
    /// 已关闭
    Closed,
}

impl std::fmt::Display for IssueState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueState::Open => write!(f, "open"),
            IssueState::Closed => write!(f, "closed"),
        }
    }
}

/// GitHub Issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubIssue {
    /// Issue 编号
    pub number: u64,
    /// Issue ID
    pub id: u64,
    /// 标题
    pub title: String,
    /// 内容
    #[serde(default)]
    pub body: Option<String>,
    /// 状态
    pub state: IssueState,
    /// 标签
    #[serde(default)]
    pub labels: Vec<Label>,
    /// 分配人
    #[serde(default)]
    pub assignees: Vec<GitHubUser>,
    /// 里程碑
    #[serde(default)]
    pub milestone: Option<Milestone>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 关闭时间
    #[serde(default)]
    pub closed_at: Option<DateTime<Utc>>,
    /// 作者
    pub user: GitHubUser,
    /// URL
    pub html_url: String,
}

/// GitHub 标签
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    /// 标签 ID
    pub id: u64,
    /// 标签名
    pub name: String,
    /// 颜色
    pub color: String,
    /// 描述
    #[serde(default)]
    pub description: Option<String>,
}

/// GitHub 里程碑
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    /// 里程碑 ID
    pub id: u64,
    /// 标题
    pub title: String,
    /// 描述
    #[serde(default)]
    pub description: Option<String>,
    /// 状态
    pub state: String,
    /// 截止日期
    #[serde(default)]
    pub due_on: Option<DateTime<Utc>>,
}

/// GitHub Pull Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubPullRequest {
    /// PR 编号
    pub number: u64,
    /// PR ID
    pub id: u64,
    /// 标题
    pub title: String,
    /// 内容
    #[serde(default)]
    pub body: Option<String>,
    /// 状态
    pub state: IssueState,
    /// 是否合并
    pub merged: bool,
    /// 基础分支
    pub base: PRBranch,
    /// 头部分支
    pub head: PRBranch,
    /// 作者
    pub user: GitHubUser,
    /// 分配人
    #[serde(default)]
    pub assignees: Vec<GitHubUser>,
    /// 请求审核人
    #[serde(default)]
    pub requested_reviewers: Vec<GitHubUser>,
    /// 标签
    #[serde(default)]
    pub labels: Vec<Label>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 合并时间
    #[serde(default)]
    pub merged_at: Option<DateTime<Utc>>,
    /// 关闭时间
    #[serde(default)]
    pub closed_at: Option<DateTime<Utc>>,
    /// URL
    pub html_url: String,
}

/// PR 分支信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PRBranch {
    /// 分支名
    #[serde(rename = "ref")]
    pub ref_name: String,
    /// SHA
    pub sha: String,
    /// 仓库
    pub repo: Repository,
}

/// 仓库信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    /// 仓库 ID
    pub id: u64,
    /// 仓库名
    pub name: String,
    /// 完整名称
    pub full_name: String,
    /// 所有者
    pub owner: GitHubUser,
    /// 是否私有
    pub private: bool,
    /// URL
    pub html_url: String,
}

/// GitHub 用户
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    /// 用户 ID
    pub id: u64,
    /// 用户名
    pub login: String,
    /// 头像 URL
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// 类型
    #[serde(rename = "type", default)]
    pub user_type: Option<String>,
}

/// 创建 Issue 请求
#[derive(Debug, Clone, Serialize)]
struct CreateIssueRequest {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assignees: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    milestone: Option<u64>,
}

/// 更新 Issue 请求
#[derive(Debug, Clone, Serialize)]
struct UpdateIssueRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assignees: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    milestone: Option<u64>,
}

/// 创建 PR 请求
#[derive(Debug, Clone, Serialize)]
struct CreatePullRequestRequest {
    title: String,
    head: String,
    base: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    draft: Option<bool>,
}

/// GitHub 客户端
pub struct GitHubClient {
    /// 配置
    config: GitHubConfig,
    /// HTTP 客户端
    http_client: reqwest::Client,
}

impl GitHubClient {
    /// 创建新客户端
    pub fn new(config: GitHubConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }

    /// 创建 Issue
    pub async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: Option<&str>,
        labels: Option<Vec<&str>>,
        assignees: Option<Vec<&str>>,
    ) -> crate::Result<GitHubIssue> {
        let url = format!("{}/repos/{}/{}/issues", self.config.api_url, owner, repo);

        let request = CreateIssueRequest {
            title: title.to_string(),
            body: body.map(|s| s.to_string()),
            labels: labels.map(|l| l.into_iter().map(|s| s.to_string()).collect()),
            assignees: assignees.map(|a| a.into_iter().map(|s| s.to_string()).collect()),
            milestone: None,
        };

        info!("Creating GitHub issue in {}/{}: {}", owner, repo, title);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("token {}", self.config.api_token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "uHorse-Integration/1.0")
            .json(&request)
            .send()
            .await
            .map_err(|e| crate::IntegrationError::GitHubError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::IntegrationError::GitHubError(format!(
                "Failed to create issue: {}",
                error_text
            )));
        }

        response
            .json()
            .await
            .map_err(|e| crate::IntegrationError::GitHubError(format!("Parse error: {}", e)))
    }

    /// 获取 Issue
    pub async fn get_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> crate::Result<GitHubIssue> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            self.config.api_url, owner, repo, issue_number
        );

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("token {}", self.config.api_token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "uHorse-Integration/1.0")
            .send()
            .await
            .map_err(|e| crate::IntegrationError::GitHubError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::IntegrationError::GitHubError(format!(
                "Issue not found: {}/{}#{}",
                owner, repo, issue_number
            )));
        }

        response
            .json()
            .await
            .map_err(|e| crate::IntegrationError::GitHubError(format!("Parse error: {}", e)))
    }

    /// 更新 Issue
    pub async fn update_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        title: Option<&str>,
        body: Option<&str>,
        state: Option<IssueState>,
        labels: Option<Vec<&str>>,
        assignees: Option<Vec<&str>>,
    ) -> crate::Result<GitHubIssue> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            self.config.api_url, owner, repo, issue_number
        );

        let request = UpdateIssueRequest {
            title: title.map(|s| s.to_string()),
            body: body.map(|s| s.to_string()),
            state: state.map(|s| s.to_string()),
            labels: labels.map(|l| l.into_iter().map(|s| s.to_string()).collect()),
            assignees: assignees.map(|a| a.into_iter().map(|s| s.to_string()).collect()),
            milestone: None,
        };

        info!("Updating GitHub issue {}/{}#{}", owner, repo, issue_number);

        let response = self
            .http_client
            .patch(&url)
            .header("Authorization", format!("token {}", self.config.api_token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "uHorse-Integration/1.0")
            .json(&request)
            .send()
            .await
            .map_err(|e| crate::IntegrationError::GitHubError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::IntegrationError::GitHubError(format!(
                "Failed to update issue: {}",
                error_text
            )));
        }

        response
            .json()
            .await
            .map_err(|e| crate::IntegrationError::GitHubError(format!("Parse error: {}", e)))
    }

    /// 关闭 Issue
    pub async fn close_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> crate::Result<GitHubIssue> {
        self.update_issue(owner, repo, issue_number, None, None, Some(IssueState::Closed), None, None)
            .await
    }

    /// 重新打开 Issue
    pub async fn reopen_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> crate::Result<GitHubIssue> {
        self.update_issue(owner, repo, issue_number, None, None, Some(IssueState::Open), None, None)
            .await
    }

    /// 创建 Pull Request
    pub async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        head: &str,
        base: &str,
        body: Option<&str>,
        draft: Option<bool>,
    ) -> crate::Result<GitHubPullRequest> {
        let url = format!(
            "{}/repos/{}/{}/pulls",
            self.config.api_url, owner, repo
        );

        let request = CreatePullRequestRequest {
            title: title.to_string(),
            head: head.to_string(),
            base: base.to_string(),
            body: body.map(|s| s.to_string()),
            draft,
        };

        info!(
            "Creating GitHub PR in {}/{}: {} ({} -> {})",
            owner, repo, title, head, base
        );

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("token {}", self.config.api_token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "uHorse-Integration/1.0")
            .json(&request)
            .send()
            .await
            .map_err(|e| crate::IntegrationError::GitHubError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::IntegrationError::GitHubError(format!(
                "Failed to create PR: {}",
                error_text
            )));
        }

        response
            .json()
            .await
            .map_err(|e| crate::IntegrationError::GitHubError(format!("Parse error: {}", e)))
    }

    /// 获取 Pull Request
    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> crate::Result<GitHubPullRequest> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.config.api_url, owner, repo, pr_number
        );

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("token {}", self.config.api_token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "uHorse-Integration/1.0")
            .send()
            .await
            .map_err(|e| crate::IntegrationError::GitHubError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::IntegrationError::GitHubError(format!(
                "PR not found: {}/{}#{}",
                owner, repo, pr_number
            )));
        }

        response
            .json()
            .await
            .map_err(|e| crate::IntegrationError::GitHubError(format!("Parse error: {}", e)))
    }

    /// 列出 Issues
    pub async fn list_issues(
        &self,
        owner: &str,
        repo: &str,
        state: Option<IssueState>,
        labels: Option<Vec<&str>>,
        limit: Option<u32>,
    ) -> crate::Result<Vec<GitHubIssue>> {
        let mut url = format!("{}/repos/{}/{}/issues", self.config.api_url, owner, repo);
        let mut params = Vec::new();

        if let Some(s) = state {
            params.push(format!("state={}", s));
        }

        if let Some(ref l) = labels {
            params.push(format!("labels={}", l.join(",")));
        }

        if let Some(limit) = limit {
            params.push(format!("per_page={}", limit));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("token {}", self.config.api_token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "uHorse-Integration/1.0")
            .send()
            .await
            .map_err(|e| crate::IntegrationError::GitHubError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::IntegrationError::GitHubError(format!(
                "Failed to list issues: {}",
                error_text
            )));
        }

        response
            .json()
            .await
            .map_err(|e| crate::IntegrationError::GitHubError(format!("Parse error: {}", e)))
    }

    /// 添加 Issue 评论
    pub async fn add_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        comment: &str,
    ) -> crate::Result<()> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}/comments",
            self.config.api_url, owner, repo, issue_number
        );

        let body = serde_json::json!({
            "body": comment
        });

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("token {}", self.config.api_token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "uHorse-Integration/1.0")
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::IntegrationError::GitHubError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::IntegrationError::GitHubError(format!(
                "Failed to add comment: {}",
                error_text
            )));
        }

        info!("Added comment to GitHub issue {}/{}#{}", owner, repo, issue_number);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_config() {
        let config = GitHubConfig::new("test-token")
            .with_default_repo("owner", "repo")
            .with_api_url("https://github.example.com/api/v3");

        assert_eq!(config.api_token, "test-token");
        assert_eq!(config.default_owner, Some("owner".to_string()));
        assert_eq!(config.default_repo, Some("repo".to_string()));
        assert_eq!(config.api_url, "https://github.example.com/api/v3");
    }

    #[test]
    fn test_issue_state_display() {
        assert_eq!(IssueState::Open.to_string(), "open");
        assert_eq!(IssueState::Closed.to_string(), "closed");
    }
}
