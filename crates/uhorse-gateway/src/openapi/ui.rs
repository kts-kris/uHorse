//! Swagger UI Integration
//!
//! 提供 Swagger UI 交互式文档界面

use axum::{
    extract::Path,
    http::{header, Response, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, Router},
};
use std::sync::Arc;

/// Swagger UI 配置
#[derive(Debug, Clone)]
pub struct SwaggerUiConfig {
    /// API 文档路径前缀
    pub path: String,
    /// OpenAPI JSON 端点
    pub api_url: String,
    /// 是否启用
    pub enabled: bool,
}

impl Default for SwaggerUiConfig {
    fn default() -> Self {
        Self {
            path: "/docs".to_string(),
            api_url: "/openapi.json".to_string(),
            enabled: true,
        }
    }
}

/// Swagger UI 服务
pub struct SwaggerUi {
    /// 配置
    config: SwaggerUiConfig,
    /// OpenAPI JSON 内容
    openapi_json: String,
}

impl SwaggerUi {
    /// 创建新的 Swagger UI 服务
    pub fn new(config: SwaggerUiConfig) -> Self {
        Self {
            config,
            openapi_json: "{}".to_string(),
        }
    }

    /// 设置 OpenAPI JSON 内容
    pub fn with_openapi(mut self, json: impl Into<String>) -> Self {
        self.openapi_json = json.into();
        self
    }

    /// 创建路由
    pub fn into_router(self) -> Router {
        if !self.config.enabled {
            return Router::new();
        }

        let openapi_json = Arc::new(self.openapi_json);
        let api_url = self.config.api_url.clone();

        Router::new()
            // OpenAPI JSON 端点
            .route(
                &api_url,
                get(move || {
                    let json = openapi_json.clone();
                    async move {
                        (
                            [(header::CONTENT_TYPE, "application/json")],
                            (*json).clone(),
                        )
                    }
                }),
            )
            // Swagger UI
            .route(
                &self.config.path,
                get(move || {
                    let url = api_url.clone();
                    async move { Html(generate_swagger_html(&url)) }
                }),
            )
    }
}

impl Default for SwaggerUi {
    fn default() -> Self {
        Self::new(SwaggerUiConfig::default())
    }
}

/// 生成 Swagger UI HTML
fn generate_swagger_html(api_url: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>uHorse API 文档</title>
    <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
    <style>
        html {{
            box-sizing: border-box;
            overflow: -moz-scrollbars-vertical;
            overflow-y: scroll;
        }}
        *, *:before, *:after {{
            box-sizing: inherit;
        }}
        body {{
            margin: 0;
            padding: 0;
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
        }}
        #swagger-ui {{
            max-width: 1460px;
            margin: 0 auto;
        }}
        .topbar {{
            display: none;
        }}
        .swagger-ui .information-container {{
            padding: 20px;
        }}
        .swagger-ui .info .title {{
            font-size: 28px;
        }}
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-standalone-preset.js"></script>
    <script>
    window.onload = function() {{
        // 初始化 Swagger UI
        const ui = SwaggerUIBundle({{
            url: "{api_url}",
            dom_id: '#swagger-ui',
            presets: [
                SwaggerUIBundle.presets.apis,
                SwaggerUIStandalonePreset
            ],
            plugins: [
                SwaggerUIBundle.plugins.DownloadUrl
            ],
            layout: "StandaloneLayout",
            // 配置选项
            deepLinking: true,
            displayOperationId: false,
            defaultModelsExpandDepth: 1,
            defaultModelExpandDepth: 1,
            displayRequestDuration: true,
            docExpansion: "list",
            filter: true,
            showExtensions: true,
            showCommonExtensions: true,
            // 授权配置
            persistAuthorization: true,
            // 语言配置
            syntaxHighlight: {{
                activate: true,
                theme: "monokai"
            }},
            // 请求配置
            tryItOutEnabled: true,
            requestSnippetsEnabled: true,
            requestSnippets: {{
                generators: {{
                    "curl_bash": {{
                        title: "cURL (bash)",
                        syntax: "bash"
                    }},
                    "curl_powershell": {{
                        title: "cURL (PowerShell)",
                        syntax: "powershell"
                    }},
                    "curl_cmd": {{
                        title: "cURL (CMD)",
                        syntax: "bash"
                    }}
                }},
                defaultExpanded: true,
                languages: ["curl_bash", "curl_powershell", "curl_cmd"]
            }}
        }});

        window.ui = ui;
    }};
    </script>
</body>
</html>"#,
        api_url = api_url
    )
}

/// ReDoc UI 配置
#[derive(Debug, Clone)]
pub struct ReDocConfig {
    /// API 文档路径
    pub path: String,
    /// OpenAPI JSON 端点
    pub api_url: String,
    /// 是否启用
    pub enabled: bool,
}

impl Default for ReDocConfig {
    fn default() -> Self {
        Self {
            path: "/redoc".to_string(),
            api_url: "/openapi.json".to_string(),
            enabled: true,
        }
    }
}

/// 生成 ReDoc HTML
pub fn generate_redoc_html(api_url: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>uHorse API 文档 - ReDoc</title>
    <style>
        body {{
            margin: 0;
            padding: 0;
        }}
    </style>
</head>
<body>
    <redoc spec-url="{api_url}"></redoc>
    <script src="https://unpkg.com/@redocly/redoc@latest/bundles/redoc.standalone.js"></script>
</body>
</html>"#,
        api_url = api_url
    )
}

/// 创建文档路由器
pub fn create_docs_router(openapi_json: String) -> Router {
    let swagger = SwaggerUi::default().with_openapi(openapi_json.clone());

    let openapi_json = Arc::new(openapi_json);
    let redoc_json = openapi_json.clone();

    Router::new()
        // OpenAPI JSON
        .route(
            "/openapi.json",
            get(move || {
                let json = openapi_json.clone();
                async move {
                    (
                        [(header::CONTENT_TYPE, "application/json")],
                        (*json).clone(),
                    )
                }
            }),
        )
        // Swagger UI
        .route(
            "/docs",
            get(|| async { Html(generate_swagger_html("/openapi.json")) }),
        )
        // ReDoc
        .route(
            "/redoc",
            get(move || async move { Html(generate_redoc_html("/openapi.json")) }),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swagger_ui_config_default() {
        let config = SwaggerUiConfig::default();
        assert_eq!(config.path, "/docs");
        assert_eq!(config.api_url, "/openapi.json");
        assert!(config.enabled);
    }

    #[test]
    fn test_generate_swagger_html() {
        let html = generate_swagger_html("/openapi.json");
        assert!(html.contains("<title>uHorse API 文档</title>"));
        assert!(html.contains("swagger-ui"));
        assert!(html.contains("/openapi.json"));
    }

    #[test]
    fn test_generate_redoc_html() {
        let html = generate_redoc_html("/openapi.json");
        assert!(html.contains("redoc"));
        assert!(html.contains("/openapi.json"));
    }
}
