//! Client Code Generator
//!
//! 从 OpenAPI 规范生成客户端代码

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 支持的客户端语言
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientLanguage {
    /// TypeScript (Axum 兼容)
    TypeScript,
    /// Go
    Go,
    /// Python
    Python,
    /// Rust
    Rust,
    /// Java
    Java,
    /// Kotlin
    Kotlin,
}

impl ClientLanguage {
    /// 获取语言名称
    pub fn name(&self) -> &'static str {
        match self {
            Self::TypeScript => "typescript",
            Self::Go => "go",
            Self::Python => "python",
            Self::Rust => "rust",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
        }
    }

    /// 获取文件扩展名
    pub fn extension(&self) -> &'static str {
        match self {
            Self::TypeScript => "ts",
            Self::Go => "go",
            Self::Python => "py",
            Self::Rust => "rs",
            Self::Java => "java",
            Self::Kotlin => "kt",
        }
    }

    /// 获取所有支持的语言
    pub fn all() -> Vec<Self> {
        vec![
            Self::TypeScript,
            Self::Go,
            Self::Python,
            Self::Rust,
            Self::Java,
            Self::Kotlin,
        ]
    }
}

/// 生成器配置
#[derive(Debug, Clone)]
pub struct GeneratorConfig {
    /// 输出目录
    pub output_dir: String,
    /// 客户端名称
    pub client_name: String,
    /// 包名/模块名
    pub package_name: String,
    /// 是否生成文档
    pub generate_docs: bool,
    /// 是否生成测试
    pub generate_tests: bool,
    /// 自定义模板变量
    pub variables: HashMap<String, String>,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            output_dir: "./generated".to_string(),
            client_name: "UhorseClient".to_string(),
            package_name: "uhorse-client".to_string(),
            generate_docs: true,
            generate_tests: false,
            variables: HashMap::new(),
        }
    }
}

/// 生成的文件
#[derive(Debug, Clone)]
pub struct GeneratedFile {
    /// 文件路径
    pub path: String,
    /// 文件内容
    pub content: String,
}

/// 生成的客户端
#[derive(Debug, Clone)]
pub struct GeneratedClient {
    /// 语言
    pub language: ClientLanguage,
    /// 生成的文件列表
    pub files: Vec<GeneratedFile>,
    /// 配置
    pub config: GeneratorConfig,
}

/// 客户端代码生成器
pub struct ClientGenerator {
    /// 配置
    config: GeneratorConfig,
    /// 目标语言
    language: ClientLanguage,
}

impl ClientGenerator {
    /// 创建新的生成器
    pub fn new(language: ClientLanguage, config: GeneratorConfig) -> Self {
        Self { config, language }
    }

    /// 从 OpenAPI JSON 生成客户端代码
    pub fn generate(&self, openapi_json: &str) -> Result<GeneratedClient, GeneratorError> {
        let _spec: serde_json::Value =
            serde_json::from_str(openapi_json).map_err(|e| GeneratorError::ParseError(e.to_string()))?;

        let mut files = Vec::new();

        // 根据语言生成客户端代码
        match self.language {
            ClientLanguage::TypeScript => {
                files.extend(self.generate_typescript(openapi_json)?);
            }
            ClientLanguage::Python => {
                files.extend(self.generate_python(openapi_json)?);
            }
            ClientLanguage::Rust => {
                files.extend(self.generate_rust(openapi_json)?);
            }
            _ => {
                // 其他语言生成基础结构
                files.push(self.generate_readme()?);
            }
        }

        Ok(GeneratedClient {
            language: self.language,
            files,
            config: self.config.clone(),
        })
    }

    /// 生成 TypeScript 客户端
    fn generate_typescript(&self, spec: &str) -> Result<Vec<GeneratedFile>, GeneratorError> {
        let mut files = Vec::new();

        // 生成类型定义
        let types_content = format!(
            r#"// Auto-generated TypeScript types from OpenAPI specification
// DO NOT EDIT MANUALLY

export interface ApiConfig {{
  baseUrl: string;
  headers?: Record<string, string>;
}}

export class {} {{
  private config: ApiConfig;

  constructor(config: ApiConfig) {{
    this.config = config;
  }}

  // API methods will be generated here
}}
"#,
            self.config.client_name
        );

        files.push(GeneratedFile {
            path: format!("{}.ts", self.config.package_name),
            content: types_content,
        });

        // 生成 README
        files.push(self.generate_readme()?);

        Ok(files)
    }

    /// 生成 Python 客户端
    fn generate_python(&self, _spec: &str) -> Result<Vec<GeneratedFile>, GeneratorError> {
        let mut files = Vec::new();

        let client_content = format!(
            r#"# Auto-generated Python client from OpenAPI specification
# DO NOT EDIT MANUALLY

from dataclasses import dataclass
from typing import Optional, Dict, Any
import httpx

@dataclass
class ApiConfig:
    base_url: str
    headers: Optional[Dict[str, str]] = None

class {}:
    def __init__(self, config: ApiConfig):
        self.config = config
        self._client = httpx.Client(base_url=config.base_url, headers=config.headers)

    # API methods will be generated here

    def close(self):
        self._client.close()
"#,
            self.config.client_name
        );

        files.push(GeneratedFile {
            path: format!("{}.py", self.config.package_name.replace("-", "_")),
            content: client_content,
        });

        files.push(self.generate_readme()?);

        Ok(files)
    }

    /// 生成 Rust 客户端
    fn generate_rust(&self, _spec: &str) -> Result<Vec<GeneratedFile>, GeneratorError> {
        let mut files = Vec::new();

        let client_content = format!(
            r#"// Auto-generated Rust client from OpenAPI specification
// DO NOT EDIT MANUALLY

use reqwest::Client;
use serde::{{Deserialize, Serialize}};

pub struct {} {{
    client: Client,
    base_url: String,
}}

impl {} {{
    pub fn new(base_url: impl Into<String>) -> Self {{
        Self {{
            client: Client::new(),
            base_url: base_url.into(),
        }}
    }}

    // API methods will be generated here
}}
"#,
            self.config.client_name, self.config.client_name
        );

        files.push(GeneratedFile {
            path: "src/lib.rs".to_string(),
            content: client_content,
        });

        files.push(self.generate_readme()?);

        Ok(files)
    }

    /// 生成 README
    fn generate_readme(&self) -> Result<GeneratedFile, GeneratorError> {
        let content = format!(
            r#"# {} Client

Auto-generated client for uHorse API.

## Installation

See language-specific instructions below.

## Usage

```{}
// Initialize the client
let client = {}::new("http://localhost:8080");

// Make API calls
// ...
```

## Generated from OpenAPI specification

This client was auto-generated from the uHorse API OpenAPI specification.
"#,
            self.config.client_name,
            self.language.extension(),
            self.config.client_name
        );

        Ok(GeneratedFile {
            path: "README.md".to_string(),
            content,
        })
    }
}

impl Default for ClientGenerator {
    fn default() -> Self {
        Self::new(ClientLanguage::TypeScript, GeneratorConfig::default())
    }
}

/// 生成器错误
#[derive(Debug, thiserror::Error)]
pub enum GeneratorError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Generation error: {0}")]
    GenerationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_language() {
        assert_eq!(ClientLanguage::TypeScript.name(), "typescript");
        assert_eq!(ClientLanguage::TypeScript.extension(), "ts");
        assert_eq!(ClientLanguage::all().len(), 6);
    }

    #[test]
    fn test_generator_config_default() {
        let config = GeneratorConfig::default();
        assert_eq!(config.client_name, "UhorseClient");
        assert!(config.generate_docs);
    }

    #[test]
    fn test_generate_typescript() {
        let generator = ClientGenerator::new(
            ClientLanguage::TypeScript,
            GeneratorConfig::default(),
        );

        let openapi = r#"{"openapi":"3.0.3","info":{"title":"Test","version":"1.0.0"}}"#;
        let result = generator.generate(openapi).unwrap();

        assert_eq!(result.language, ClientLanguage::TypeScript);
        assert!(!result.files.is_empty());
    }
}
