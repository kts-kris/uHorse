//! # uHorse Integration Module
//!
//! 第三方系统集成模块
//!
//! ## Features
//!
//! - Jira 集成 (工单创建/更新)
//! - GitHub 集成 (Issue/PR 管理)
//! - Slack 通知 (消息推送)

pub mod github;
pub mod jira;
pub mod slack;

pub use github::{
    GitHubClient, GitHubConfig, GitHubIssue, GitHubPullRequest, GitHubUser, IssueState, Label,
    Milestone, PRBranch, Repository,
};
pub use jira::{JiraClient, JiraConfig, JiraIssue, JiraPriority, JiraStatus, JiraUser};
pub use slack::{
    AlertSeverity, SlackAttachment, SlackChannel, SlackClient, SlackConfig, SlackField,
    SlackMessage, SlackUser,
};

use thiserror::Error;

/// 集成错误类型
#[derive(Error, Debug)]
pub enum IntegrationError {
    #[error("Jira error: {0}")]
    JiraError(String),

    #[error("GitHub error: {0}")]
    GitHubError(String),

    #[error("Slack error: {0}")]
    SlackError(String),

    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// 集成结果类型
pub type Result<T> = std::result::Result<T, IntegrationError>;
