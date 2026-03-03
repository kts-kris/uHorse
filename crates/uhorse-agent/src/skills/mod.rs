//! # Skills - 技能系统（OpenClaw 风格）
//!
//! OpenClaw 风格的技能系统，每个技能包含：
//! - SKILL.md - AI 可读的技能描述
//! - 执行逻辑 - Rust/WASM 实现
//! - 配置文件 - skill.toml
//!
//! ## 技能结构
//!
//! ```text
//! workspace/skills/my-skill/
//! ├── SKILL.md          # 技能描述（AI 阅读）
//! ├── mod.rs            # Rust 执行逻辑
//! ├── skill.toml        # 技能配置
//! └── examples/         # 使用示例
//! ```

use crate::error::{AgentError, AgentResult};
use crate::mcp::types::{McpContent, McpTool, McpToolCall, McpToolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// SKILL.md 解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    /// 技能名称
    pub name: String,
    /// 技能描述
    pub description: String,
    /// 技能版本
    pub version: String,
    /// 作者
    pub author: Option<String>,
    /// 标签
    pub tags: Vec<String>,
    /// MCP 工具定义
    pub tools: Vec<McpTool>,
    /// MCP 资源定义
    pub resources: Vec<String>,
    /// 依赖的其他技能
    pub dependencies: Vec<String>,
}

/// 技能配置（skill.toml）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfig {
    /// 技能名称
    pub name: String,
    /// 是否启用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 权限级别
    #[serde(default = "default_permission")]
    pub permission: SkillPermission,
    /// 速率限制（每分钟调用次数）
    #[serde(default)]
    pub rate_limit: Option<usize>,
}

fn default_enabled() -> bool {
    true
}

fn default_permission() -> SkillPermission {
    SkillPermission::Normal
}

/// 技能权限级别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SkillPermission {
    /// 只读
    ReadOnly,
    /// 正常（可读写）
    Normal,
    /// 危险（可修改系统状态）
    Dangerous,
}

/// 技能执行器 trait
#[async_trait::async_trait]
pub trait SkillExecutor: Send + Sync {
    /// 执行工具调用
    async fn execute_tool(&self, call: &McpToolCall) -> AgentResult<McpToolResult>;

    /// 获取技能清单
    fn manifest(&self) -> &SkillManifest;

    /// 技能配置
    fn config(&self) -> &SkillConfig;

    /// 初始化技能
    async fn initialize(&mut self) -> AgentResult<()> {
        Ok(())
    }

    /// 清理资源
    async fn cleanup(&mut self) -> AgentResult<()> {
        Ok(())
    }
}

/// 技能定义
#[derive(Clone)]
pub struct Skill {
    /// 技能清单
    manifest: SkillManifest,
    /// 技能配置
    config: SkillConfig,
    /// 执行器
    executor: Arc<dyn SkillExecutor>,
}

impl Skill {
    /// 创建新技能
    pub fn new(
        manifest: SkillManifest,
        config: SkillConfig,
        executor: Arc<dyn SkillExecutor>,
    ) -> Self {
        Self {
            manifest,
            config,
            executor,
        }
    }

    /// 获取技能名称
    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    /// 获取技能描述
    pub fn description(&self) -> &str {
        &self.manifest.description
    }

    /// 获取技能清单
    pub fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    /// 获取技能配置
    pub fn config(&self) -> &SkillConfig {
        &self.config
    }

    /// 是否启用
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// 执行工具调用
    pub async fn execute_tool(&self, call: &McpToolCall) -> AgentResult<McpToolResult> {
        if !self.is_enabled() {
            return Err(AgentError::Skill(format!(
                "Skill '{}' is disabled",
                self.name()
            )));
        }

        self.executor.execute_tool(call).await
    }

    /// 获取所有工具
    pub fn tools(&self) -> &[McpTool] {
        &self.manifest.tools
    }

    /// 获取指定工具
    pub fn get_tool(&self, name: &str) -> Option<&McpTool> {
        self.manifest.tools.iter().find(|t| t.name == name)
    }
}

/// SKILL.md 解析器
pub struct SkillManifestParser;

impl SkillManifestParser {
    /// 从 SKILL.md 文件解析清单
    pub async fn parse_from_file(path: &PathBuf) -> AgentResult<SkillManifest> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| AgentError::Skill(format!("Failed to read SKILL.md: {}", e)))?;

        Self::parse_from_content(&content)
    }

    /// 从内容解析清单
    pub fn parse_from_content(content: &str) -> AgentResult<SkillManifest> {
        let mut name = String::new();
        let mut description = String::new();
        let mut version = "1.0.0".to_string();
        let mut author = None;
        let mut tags = Vec::new();
        let mut tools = Vec::new();

        // 简单的 Markdown 解析（生产环境可以使用更强大的解析器）
        for line in content.lines() {
            if line.starts_with("# ") {
                // 标题可能是技能名称
                let title = &line[2..];
                if name.is_empty() {
                    name = title.to_string();
                }
            } else if line.to_lowercase().starts_with("## description") {
                // 描述
                description = extract_next_paragraph(content, line);
            } else if line.to_lowercase().starts_with("## version") {
                version = extract_value(content, line);
            } else if line.to_lowercase().starts_with("## author") {
                author = Some(extract_value(content, line));
            } else if line.to_lowercase().starts_with("## tags") {
                let tags_str = extract_value(content, line);
                tags = tags_str.split(',').map(|s| s.trim().to_string()).collect();
            } else if line.to_lowercase().starts_with("## tools") {
                // 解析工具定义（JSON 格式）
                // 找到当前行在整个 content 中的位置
                let line_pos = content.find(line).unwrap_or(0);
                let after_tools = &content[line_pos + line.len()..];

                // 查找 JSON 对象
                if let Some(json_start) = after_tools.find('{') {
                    let remaining = &after_tools[json_start..];

                    // 找到匹配的闭合括号（使用字节位置）
                    let mut brace_count = 0;
                    let mut json_end = 0;
                    for (byte_pos, byte) in remaining.bytes().enumerate() {
                        if byte == b'{' {
                            brace_count += 1;
                        } else if byte == b'}' {
                            brace_count -= 1;
                            if brace_count == 0 {
                                json_end = byte_pos;
                                break;
                            }
                        }
                    }

                    if json_end > 0 {
                        if let Some(json_str) = remaining.get(..=json_end) {
                            if let Ok(tool) = serde_json::from_str::<McpTool>(json_str) {
                                tools.push(tool);
                            }
                        }
                    }
                }
            }
        }

        Ok(SkillManifest {
            name,
            description,
            version,
            author,
            tags,
            tools,
            resources: Vec::new(),
            dependencies: Vec::new(),
        })
    }
}

/// 提取下一段落
fn extract_next_paragraph(content: &str, after_line: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut found = false;
    let mut paragraph = Vec::new();

    for line in lines {
        if found {
            if line.is_empty() {
                break;
            }
            if !line.starts_with('#') {
                paragraph.push(line.trim());
            }
        } else if line == after_line {
            found = true;
        }
    }

    paragraph.join(" ")
}

/// 提取值（用于 key: value 格式）
fn extract_value(content: &str, key_line: &str) -> String {
    if let Some(rest) = key_line.split(':').nth(1) {
        return rest.trim().to_string();
    }

    // 否则查找下一行
    let lines: Vec<&str> = content.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if *line == key_line {
            if let Some(next_line) = lines.get(i + 1) {
                return next_line.trim().to_string();
            }
        }
    }

    String::new()
}

/// 技能注册表
#[derive(Clone)]
pub struct SkillRegistry {
    /// 技能映射
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    /// 创建新的注册表
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// 注册技能
    pub fn register(&mut self, skill: Skill) {
        let name = skill.name().to_string();
        self.skills.insert(name, skill);
    }

    /// 获取技能
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// 列出所有技能名称
    pub fn list_names(&self) -> Vec<String> {
        self.skills.keys().cloned().collect()
    }

    /// 列出所有启用的技能
    pub fn list_enabled(&self) -> Vec<&Skill> {
        self.skills.values().filter(|s| s.is_enabled()).collect()
    }

    /// 获取所有工具（来自所有启用的技能）
    pub fn get_all_tools(&self) -> Vec<McpTool> {
        self.skills
            .values()
            .filter(|s| s.is_enabled())
            .flat_map(|s| s.tools().to_vec())
            .collect()
    }

    /// 根据工具名查找所属技能
    pub fn find_skill_for_tool(&self, tool_name: &str) -> Option<&Skill> {
        self.skills
            .values()
            .find(|s| s.get_tool(tool_name).is_some())
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_registry() {
        let mut registry = SkillRegistry::new();

        let manifest = SkillManifest {
            name: "test".to_string(),
            description: "Test skill".to_string(),
            version: "1.0.0".to_string(),
            author: None,
            tags: vec![],
            tools: vec![],
            resources: vec![],
            dependencies: vec![],
        };

        let config = SkillConfig {
            name: "test".to_string(),
            enabled: true,
            permission: SkillPermission::Normal,
            rate_limit: None,
        };

        let skill = Skill::new(manifest, config, Arc::new(DummyExecutor));

        registry.register(skill);

        assert!(registry.get("test").is_some());
    }

    #[test]
    fn test_skill_manifest_parse() {
        let content = r#"
# Test Skill

## Description
This is a test skill for demonstration.

## Version
1.0.0

## Tags
test,demo
"#;

        let manifest = SkillManifestParser::parse_from_content(content).unwrap();
        assert_eq!(manifest.name, "Test Skill");
        assert_eq!(manifest.version, "1.0.0");
    }
}

/// 测试用执行器
struct DummyExecutor;

#[async_trait::async_trait]
impl SkillExecutor for DummyExecutor {
    async fn execute_tool(&self, _call: &McpToolCall) -> AgentResult<McpToolResult> {
        Ok(McpToolResult {
            name: "dummy".to_string(),
            content: vec![McpContent::Text {
                text: "Dummy result".to_string(),
            }],
            is_error: false,
        })
    }

    fn manifest(&self) -> &SkillManifest {
        // 返回一个静态的 manifest
        static MANIFEST: std::sync::OnceLock<SkillManifest> = std::sync::OnceLock::new();
        MANIFEST.get_or_init(|| SkillManifest {
            name: "dummy".to_string(),
            description: "Dummy skill".to_string(),
            version: "1.0.0".to_string(),
            author: None,
            tags: vec![],
            tools: vec![],
            resources: vec![],
            dependencies: vec![],
        })
    }

    fn config(&self) -> &SkillConfig {
        static CONFIG: std::sync::OnceLock<SkillConfig> = std::sync::OnceLock::new();
        CONFIG.get_or_init(|| SkillConfig {
            name: "dummy".to_string(),
            enabled: true,
            permission: SkillPermission::Normal,
            rate_limit: None,
        })
    }
}
