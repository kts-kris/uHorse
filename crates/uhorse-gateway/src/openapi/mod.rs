//! OpenAPI Specification Module
//!
//! OpenAPI 3.0 规范生成和 Swagger UI 集成

mod spec;
mod ui;
mod generator;

pub use spec::{OpenApiInfo, OpenApiManager, ServerInfo, SecurityScheme, TagInfo};
pub use ui::{SwaggerUi, SwaggerUiConfig, create_docs_router};
pub use generator::{ClientGenerator, ClientLanguage, GeneratorConfig, GeneratedClient, GeneratedFile};

use serde::{Deserialize, Serialize};

/// OpenAPI 文档配置
#[derive(Debug, Clone)]
pub struct OpenApiConfig {
    /// API 标题
    pub title: String,
    /// API 描述
    pub description: String,
    /// API 版本
    pub version: String,
    /// 服务器列表
    pub servers: Vec<ServerConfig>,
    /// 是否启用 Swagger UI
    pub enable_swagger: bool,
    /// Swagger UI 路径
    pub swagger_path: String,
    /// OpenAPI JSON 路径
    pub spec_path: String,
}

impl Default for OpenApiConfig {
    fn default() -> Self {
        Self {
            title: "uHorse API".to_string(),
            description: "企业级多渠道 AI 网关 API".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            servers: vec![ServerConfig::default()],
            enable_swagger: true,
            swagger_path: "/docs".to_string(),
            spec_path: "/openapi.json".to_string(),
        }
    }
}

/// 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 服务器 URL
    pub url: String,
    /// 服务器描述
    pub description: String,
    /// 环境变量
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8080".to_string(),
            description: "本地开发服务器".to_string(),
            variables: std::collections::HashMap::new(),
        }
    }
}

impl ServerConfig {
    /// 创建新的服务器配置
    pub fn new(url: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            description: description.into(),
            variables: std::collections::HashMap::new(),
        }
    }

    /// 添加环境变量
    pub fn with_variable(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables.insert(name.into(), value.into());
        self
    }
}
