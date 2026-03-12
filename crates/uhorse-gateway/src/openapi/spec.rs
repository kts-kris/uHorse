//! OpenAPI Specification Generation
//!
//! 使用 utoipa 生成 OpenAPI 3.0 规范

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use std::collections::HashMap;

use super::ServerConfig;

/// OpenAPI 文档信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiInfo {
    /// API 标题
    pub title: String,
    /// API 描述
    pub description: String,
    /// API 版本
    pub version: String,
    /// 联系信息
    pub contact: Option<ContactInfo>,
    /// 许可证
    pub license: Option<LicenseInfo>,
}

impl Default for OpenApiInfo {
    fn default() -> Self {
        Self {
            title: "uHorse AI Gateway API".to_string(),
            description: "企业级多渠道 AI 网关 API".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            contact: Some(ContactInfo {
                name: "uHorse Team".to_string(),
                email: "support@uhorse.ai".to_string(),
                url: "https://uhorse.ai".to_string(),
            }),
            license: Some(LicenseInfo {
                name: "MIT OR Apache-2.0".to_string(),
                url: "https://opensource.org/licenses/MIT".to_string(),
            }),
        }
    }
}

/// 联系信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactInfo {
    /// 联系人名称
    pub name: String,
    /// 联系邮箱
    pub email: String,
    /// 联系网址
    pub url: String,
}

/// 许可证信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseInfo {
    /// 许可证名称
    pub name: String,
    /// 许可证网址
    pub url: String,
}

/// OpenAPI 服务器信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    /// 服务器 URL
    pub url: String,
    /// 服务器描述
    pub description: String,
    /// 服务器变量
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub variables: HashMap<String, ServerVariable>,
}

/// 服务器变量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerVariable {
    /// 变量描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 默认值
    pub default: String,
    /// 枚举值列表
    #[serde(rename = "enum", skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<String>,
}

/// OpenAPI 安全方案
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScheme {
    /// 方案类型
    #[serde(rename = "type")]
    pub scheme_type: String,
    /// 方案描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 方案名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 位置 (header/query/cookie)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "in")]
    pub location: Option<String>,
    /// Bearer 格式
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "bearerFormat")]
    pub bearer_format: Option<String>,
}

impl SecurityScheme {
    /// 创建 JWT Bearer 安全方案
    pub fn jwt_bearer() -> Self {
        Self {
            scheme_type: "http".to_string(),
            description: Some("JWT 认证".to_string()),
            name: Some("Authorization".to_string()),
            location: Some("header".to_string()),
            bearer_format: Some("Bearer".to_string()),
        }
    }

    /// 创建 API Key 安全方案
    pub fn api_key(name: impl Into<String>) -> Self {
        Self {
            scheme_type: "apiKey".to_string(),
            description: Some("API Key 认证".to_string()),
            name: Some(name.into()),
            location: Some("header".to_string()),
            bearer_format: None,
        }
    }
}

/// OpenAPI 标签
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagInfo {
    /// 标签名称
    pub name: String,
    /// 标签描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 外部文档
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "externalDocs")]
    pub external_docs: Option<ExternalDocs>,
}

/// 外部文档
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalDocs {
    /// 文档 URL
    pub url: String,
    /// 文档描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// OpenAPI 规范管理器
pub struct OpenApiManager {
    /// 文档信息
    info: OpenApiInfo,
    /// 服务器列表
    servers: Vec<ServerInfo>,
    /// 安全方案
    security_schemes: HashMap<String, SecurityScheme>,
    /// 标签
    tags: Vec<TagInfo>,
    /// 外部文档
    external_docs: Option<ExternalDocs>,
}

impl OpenApiManager {
    /// 创建新的 OpenAPI 管理器
    pub fn new(info: OpenApiInfo) -> Self {
        Self {
            info,
            servers: Vec::new(),
            security_schemes: HashMap::new(),
            tags: Vec::new(),
            external_docs: None,
        }
    }

    /// 添加服务器
    pub fn add_server(&mut self, server: ServerInfo) -> &mut Self {
        self.servers.push(server);
        self
    }

    /// 添加安全方案
    pub fn add_security_scheme(&mut self, name: impl Into<String>, scheme: SecurityScheme) -> &mut Self {
        self.security_schemes.insert(name.into(), scheme);
        self
    }

    /// 添加标签
    pub fn add_tag(&mut self, tag: TagInfo) -> &mut Self {
        self.tags.push(tag);
        self
    }

    /// 设置外部文档
    pub fn set_external_docs(&mut self, docs: ExternalDocs) -> &mut Self {
        self.external_docs = Some(docs);
        self
    }

    /// 生成 OpenAPI JSON
    pub fn to_json(&self) -> serde_json::Value {
        let mut doc = serde_json::Map::new();

        // OpenAPI 版本
        doc.insert("openapi".to_string(), serde_json::json!("3.0.3"));

        // 基本信息
        doc.insert("info".to_string(), serde_json::to_value(&self.info).unwrap());

        // 服务器列表
        if !self.servers.is_empty() {
            doc.insert("servers".to_string(), serde_json::to_value(&self.servers).unwrap());
        }

        // 安全方案
        if !self.security_schemes.is_empty() {
            let mut components = serde_json::Map::new();
            components.insert(
                "securitySchemes".to_string(),
                serde_json::to_value(&self.security_schemes).unwrap(),
            );
            doc.insert("components".to_string(), serde_json::Value::Object(components));
        }

        // 标签
        if !self.tags.is_empty() {
            doc.insert("tags".to_string(), serde_json::to_value(&self.tags).unwrap());
        }

        // 外部文档
        if let Some(ref docs) = self.external_docs {
            doc.insert("externalDocs".to_string(), serde_json::to_value(docs).unwrap());
        }

        serde_json::Value::Object(doc)
    }

    /// 生成 OpenAPI YAML
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(&self.to_json())
    }

    /// 获取默认标签
    pub fn default_tags() -> Vec<TagInfo> {
        vec![
            TagInfo {
                name: "health".to_string(),
                description: Some("健康检查端点".to_string()),
                external_docs: None,
            },
            TagInfo {
                name: "auth".to_string(),
                description: Some("认证授权端点".to_string()),
                external_docs: None,
            },
            TagInfo {
                name: "agents".to_string(),
                description: Some("智能体管理端点".to_string()),
                external_docs: None,
            },
            TagInfo {
                name: "skills".to_string(),
                description: Some("技能管理端点".to_string()),
                external_docs: None,
            },
            TagInfo {
                name: "sessions".to_string(),
                description: Some("会话管理端点".to_string()),
                external_docs: None,
            },
            TagInfo {
                name: "files".to_string(),
                description: Some("文件管理端点".to_string()),
                external_docs: None,
            },
            TagInfo {
                name: "channels".to_string(),
                description: Some("通道管理端点".to_string()),
                external_docs: None,
            },
            TagInfo {
                name: "system".to_string(),
                description: Some("系统管理端点".to_string()),
                external_docs: None,
            },
        ]
    }
}

impl Default for OpenApiManager {
    fn default() -> Self {
        let mut manager = Self::new(OpenApiInfo::default());

        // 添加默认服务器
        manager.add_server(ServerInfo {
            url: "http://localhost:8080".to_string(),
            description: "本地开发服务器".to_string(),
            variables: HashMap::new(),
        });

        // 添加默认安全方案
        manager.add_security_scheme("bearerAuth", SecurityScheme::jwt_bearer());

        // 添加默认标签
        for tag in Self::default_tags() {
            manager.add_tag(tag);
        }

        manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_info_default() {
        let info = OpenApiInfo::default();
        assert_eq!(info.title, "uHorse AI Gateway API");
        assert!(info.contact.is_some());
        assert!(info.license.is_some());
    }

    #[test]
    fn test_security_scheme_jwt() {
        let scheme = SecurityScheme::jwt_bearer();
        assert_eq!(scheme.scheme_type, "http");
        assert_eq!(scheme.bearer_format, Some("Bearer".to_string()));
    }

    #[test]
    fn test_openapi_manager() {
        let manager = OpenApiManager::default();
        let json = manager.to_json();

        assert_eq!(json["openapi"], "3.0.3");
        assert!(json["info"].is_object());
        assert!(json["servers"].is_array());
    }

    #[test]
    fn test_openapi_to_yaml() {
        let manager = OpenApiManager::default();
        let yaml = manager.to_yaml().unwrap();

        assert!(yaml.contains("openapi: \"3.0.3\"") || yaml.contains("openapi: '3.0.3'") || yaml.contains("openapi: 3.0.3"));
        assert!(yaml.contains("title: uHorse AI Gateway API") || yaml.contains("title: \"uHorse AI Gateway API\""));
    }
}
