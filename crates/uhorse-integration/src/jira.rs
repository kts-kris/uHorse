//! Jira Integration
//!
//! Jira 工单系统集成

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;

/// Jira 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraConfig {
    /// Jira 站点 URL
    pub site_url: String,
    /// API Token
    pub api_token: String,
    /// 用户邮箱
    pub email: String,
    /// 项目 Key
    pub project_key: String,
    /// Issue Type ID
    #[serde(default = "default_issue_type")]
    pub issue_type_id: String,
}

fn default_issue_type() -> String {
    "10001".to_string() // Task
}

impl JiraConfig {
    /// 创建新配置
    pub fn new(
        site_url: impl Into<String>,
        email: impl Into<String>,
        api_token: impl Into<String>,
        project_key: impl Into<String>,
    ) -> Self {
        Self {
            site_url: site_url.into(),
            api_token: api_token.into(),
            email: email.into(),
            project_key: project_key.into(),
            issue_type_id: default_issue_type(),
        }
    }

    /// 获取认证头
    pub fn auth_header(&self) -> String {
        let credentials = format!("{}:{}", self.email, self.api_token);
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            credentials.as_bytes(),
        );
        format!("Basic {}", encoded)
    }
}

/// Jira Issue 优先级
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JiraPriority {
    /// 最高
    Highest,
    /// 高
    High,
    /// 中
    Medium,
    /// 低
    Low,
    /// 最低
    Lowest,
}

impl std::fmt::Display for JiraPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JiraPriority::Highest => write!(f, "Highest"),
            JiraPriority::High => write!(f, "High"),
            JiraPriority::Medium => write!(f, "Medium"),
            JiraPriority::Low => write!(f, "Low"),
            JiraPriority::Lowest => write!(f, "Lowest"),
        }
    }
}

/// Jira Issue 状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraStatus {
    /// 状态 ID
    pub id: String,
    /// 状态名称
    pub name: String,
}

/// Jira Issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraIssue {
    /// Issue Key
    pub key: String,
    /// Issue ID
    pub id: String,
    /// 标题
    #[serde(rename = "summary")]
    pub summary: String,
    /// 描述
    #[serde(default)]
    pub description: Option<String>,
    /// 状态
    pub status: JiraStatus,
    /// 优先级
    #[serde(default)]
    pub priority: Option<JiraPriority>,
    /// 创建时间
    #[serde(default)]
    pub created: Option<DateTime<Utc>>,
    /// 更新时间
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
    /// 报告人
    #[serde(default)]
    pub reporter: Option<JiraUser>,
    /// 分配人
    #[serde(default)]
    pub assignee: Option<JiraUser>,
}

/// Jira 用户
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraUser {
    /// 账户 ID
    #[serde(rename = "accountId")]
    pub account_id: String,
    /// 显示名称
    #[serde(rename = "displayName")]
    pub display_name: String,
    /// 邮箱
    #[serde(default)]
    pub email: Option<String>,
}

/// 创建 Issue 请求
#[derive(Debug, Clone, Serialize)]
struct CreateIssueRequest {
    fields: CreateIssueFields,
}

#[derive(Debug, Clone, Serialize)]
struct CreateIssueFields {
    project: ProjectRef,
    summary: String,
    description: Option<Description>,
    issuetype: IssueTypeRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<PriorityRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assignee: Option<AccountRef>,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectRef {
    key: String,
}

#[derive(Debug, Clone, Serialize)]
struct IssueTypeRef {
    id: String,
}

#[derive(Debug, Clone, Serialize)]
struct PriorityRef {
    name: String,
}

#[derive(Debug, Clone, Serialize)]
struct AccountRef {
    account_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct Description {
    #[serde(rename = "type")]
    doc_type: String,
    version: u32,
    content: Vec<Content>,
}

#[derive(Debug, Clone, Serialize)]
struct Content {
    #[serde(rename = "type")]
    content_type: String,
    content: Vec<Paragraph>,
}

#[derive(Debug, Clone, Serialize)]
struct Paragraph {
    #[serde(rename = "type")]
    para_type: String,
    text: String,
}

/// 创建 Issue 响应
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct CreateIssueResponse {
    id: String,
    key: String,
    #[serde(rename = "self")]
    url: String,
}

/// Jira 客户端
pub struct JiraClient {
    /// 配置
    config: JiraConfig,
    /// HTTP 客户端
    http_client: reqwest::Client,
}

impl JiraClient {
    /// 创建新客户端
    pub fn new(config: JiraConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }

    /// 创建 Issue
    pub async fn create_issue(
        &self,
        summary: &str,
        description: Option<&str>,
        priority: Option<JiraPriority>,
        assignee_id: Option<&str>,
    ) -> crate::Result<JiraIssue> {
        let url = format!("{}/rest/api/3/issue", self.config.site_url);

        let desc = description.map(|d| Description {
            doc_type: "doc".to_string(),
            version: 1,
            content: vec![Content {
                content_type: "paragraph".to_string(),
                content: vec![Paragraph {
                    para_type: "text".to_string(),
                    text: d.to_string(),
                }],
            }],
        });

        let request = CreateIssueRequest {
            fields: CreateIssueFields {
                project: ProjectRef {
                    key: self.config.project_key.clone(),
                },
                summary: summary.to_string(),
                description: desc,
                issuetype: IssueTypeRef {
                    id: self.config.issue_type_id.clone(),
                },
                priority: priority.map(|p| PriorityRef {
                    name: p.to_string(),
                }),
                assignee: assignee_id.map(|id| AccountRef {
                    account_id: id.to_string(),
                }),
            },
        };

        info!("Creating Jira issue: {}", summary);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", self.config.auth_header())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| crate::IntegrationError::JiraError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::IntegrationError::JiraError(format!(
                "Failed to create issue: {}",
                error_text
            )));
        }

        let created: CreateIssueResponse = response
            .json()
            .await
            .map_err(|e| crate::IntegrationError::JiraError(format!("Parse error: {}", e)))?;

        // 获取完整的 Issue 信息
        self.get_issue(&created.key).await
    }

    /// 获取 Issue
    pub async fn get_issue(&self, issue_key: &str) -> crate::Result<JiraIssue> {
        let url = format!("{}/rest/api/3/issue/{}", self.config.site_url, issue_key);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", self.config.auth_header())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| crate::IntegrationError::JiraError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::IntegrationError::JiraError(format!(
                "Issue not found: {}",
                issue_key
            )));
        }

        let issue_response: JiraIssueResponse = response
            .json()
            .await
            .map_err(|e| crate::IntegrationError::JiraError(format!("Parse error: {}", e)))?;

        Ok(JiraIssue {
            key: issue_response.key,
            id: issue_response.id,
            summary: issue_response.fields.summary,
            description: issue_response.fields.description,
            status: issue_response.fields.status,
            priority: None,
            created: None,
            updated: None,
            reporter: None,
            assignee: issue_response.fields.assignee,
        })
    }

    /// 更新 Issue 状态
    pub async fn transition_issue(
        &self,
        issue_key: &str,
        transition_id: &str,
    ) -> crate::Result<()> {
        let url = format!(
            "{}/rest/api/3/issue/{}/transitions",
            self.config.site_url, issue_key
        );

        let body = serde_json::json!({
            "transition": {
                "id": transition_id
            }
        });

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", self.config.auth_header())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::IntegrationError::JiraError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::IntegrationError::JiraError(format!(
                "Failed to transition issue: {}",
                error_text
            )));
        }

        info!("Transitioned Jira issue {} to {}", issue_key, transition_id);
        Ok(())
    }

    /// 添加评论
    pub async fn add_comment(&self, issue_key: &str, comment: &str) -> crate::Result<()> {
        let url = format!(
            "{}/rest/api/3/issue/{}/comment",
            self.config.site_url, issue_key
        );

        let body = serde_json::json!({
            "body": {
                "type": "doc",
                "version": 1,
                "content": [{
                    "type": "paragraph",
                    "content": [{
                        "type": "text",
                        "text": comment
                    }]
                }]
            }
        });

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", self.config.auth_header())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::IntegrationError::JiraError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(crate::IntegrationError::JiraError(format!(
                "Failed to add comment: {}",
                error_text
            )));
        }

        info!("Added comment to Jira issue {}", issue_key);
        Ok(())
    }
}

/// Issue 响应
#[derive(Debug, Clone, Deserialize)]
struct JiraIssueResponse {
    key: String,
    id: String,
    fields: JiraIssueFields,
}

#[derive(Debug, Clone, Deserialize)]
struct JiraIssueFields {
    summary: String,
    #[serde(default)]
    description: Option<String>,
    status: JiraStatus,
    #[serde(default)]
    assignee: Option<JiraUser>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jira_config() {
        let config = JiraConfig::new(
            "https://example.atlassian.net",
            "user@example.com",
            "api-token",
            "PROJ",
        );

        assert_eq!(config.site_url, "https://example.atlassian.net");
        assert_eq!(config.project_key, "PROJ");
        assert!(config.auth_header().starts_with("Basic "));
    }

    #[test]
    fn test_priority_display() {
        assert_eq!(JiraPriority::Highest.to_string(), "Highest");
        assert_eq!(JiraPriority::Medium.to_string(), "Medium");
        assert_eq!(JiraPriority::Low.to_string(), "Low");
    }
}
