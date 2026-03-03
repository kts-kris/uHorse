//! # Skills - 技能系统
//!
//! OpenClaw 风格的技能系统，每个技能包含 SKILL.md 描述和执行逻辑。
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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// 技能清单（SKILL.md）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    /// 技能名称
    pub name: String,
    /// 技能描述
    pub description: String,
    /// 版本
    pub version: String,
    /// 作者
    pub author: Option<String>,
    /// 参数定义
    pub parameters: Vec<SkillParameter>,
    /// 所需权限
    pub permissions: Vec<String>,
}

/// 技能参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParameter {
    /// 参数名
    pub name: String,
    /// 参数描述
    pub description: String,
    /// 参数类型
    pub param_type: String,
    /// 是否必需
    pub required: bool,
    /// 默认值
    pub default: Option<serde_json::Value>,
}

/// 技能配置（skill.toml）
#[derive(Debug, Clone, Deserialize)]
pub struct SkillConfig {
    /// 技能启用状态
    #[serde(default)]
    pub enabled: bool,
    /// 超时时间（秒）
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    /// 最大重试次数
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,
}

impl Default for SkillConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout: default_timeout(),
            max_retries: default_max_retries(),
        }
    }
}

fn default_timeout() -> u64 {
    30
}
fn default_max_retries() -> usize {
    3
}

/// 技能执行器
#[async_trait::async_trait]
pub trait SkillExecutor: Send + Sync {
    /// 执行技能
    async fn execute(&self, input: &str) -> AgentResult<String>;

    /// 获取技能名称
    fn name(&self) -> &str;

    /// 获取技能描述
    fn description(&self) -> &str;
}

/// 技能
#[derive(Clone)]
pub struct Skill {
    /// 清单
    pub manifest: SkillManifest,
    /// 配置
    pub config: SkillConfig,
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

    /// 执行技能
    pub async fn execute(&self, input: &str) -> AgentResult<String> {
        if !self.config.enabled {
            return Err(AgentError::Skill(format!(
                "Skill {} is disabled",
                self.manifest.name
            )));
        }

        self.executor.execute(input).await
    }

    /// 获取技能名称
    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    /// 获取技能描述
    pub fn description(&self) -> &str {
        &self.manifest.description
    }
}

/// 技能注册表
#[derive(Clone, Default)]
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    /// 创建新的注册表
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册技能
    pub fn register(&mut self, skill: Skill) {
        let name = skill.name().to_string();
        self.skills.insert(name, skill);
    }

    /// 获取技能
    pub fn get(&self, name: &str) -> Option<Skill> {
        self.skills.get(name).cloned()
    }

    /// 列出所有技能名称
    pub fn list_names(&self) -> Vec<String> {
        self.skills.keys().cloned().collect()
    }

    /// 加载技能目录
    pub async fn load_from_dir(&mut self, dir: PathBuf) -> AgentResult<usize> {
        let mut count = 0;

        let entries = tokio::fs::read_dir(&dir).await?;
        let mut read_dir = entries;

        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();

            if path.is_dir() {
                // 尝试加载技能
                if let Ok(skill) = Self::load_skill(&path).await {
                    self.register(skill);
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// 从目录加载单个技能
    async fn load_skill(dir: &PathBuf) -> AgentResult<Skill> {
        // 读取 SKILL.md
        let skill_md_path = dir.join("SKILL.md");
        let skill_md_content = tokio::fs::read_to_string(&skill_md_path).await?;

        // 解析清单
        let manifest = Self::parse_skill_md(&skill_md_content)?;

        // 读取 skill.toml
        let skill_toml_path = dir.join("skill.toml");
        let config = if skill_toml_path.exists() {
            let toml_content = tokio::fs::read_to_string(&skill_toml_path).await?;
            toml::from_str(&toml_content)
                .map_err(|e| AgentError::Skill(format!("Failed to parse skill.toml: {}", e)))?
        } else {
            SkillConfig::default()
        };

        // 创建执行器（这里简化，实际需要动态加载）
        let executor = Arc::new(DummySkillExecutor {
            name: manifest.name.clone(),
            description: manifest.description.clone(),
        });

        Ok(Skill::new(manifest, config, executor))
    }

    /// 解析 SKILL.md
    fn parse_skill_md(content: &str) -> AgentResult<SkillManifest> {
        // 简单解析：提取 YAML frontmatter
        let content = content.trim();

        if !content.starts_with("---") {
            return Err(AgentError::Skill(
                "SKILL.md must start with YAML frontmatter".to_string(),
            ));
        }

        let end_idx = content[3..]
            .find("---")
            .ok_or_else(|| AgentError::Skill("Invalid YAML frontmatter".to_string()))?;

        let yaml_content = &content[3..end_idx + 3];
        let manifest: SkillManifest = serde_yaml::from_str(yaml_content)
            .map_err(|e| AgentError::Skill(format!("Failed to parse SKILL.md: {}", e)))?;

        Ok(manifest)
    }
}

/// 哑技能执行器（用于测试）
struct DummySkillExecutor {
    name: String,
    description: String,
}

#[async_trait::async_trait]
impl SkillExecutor for DummySkillExecutor {
    async fn execute(&self, input: &str) -> AgentResult<String> {
        Ok(format!(
            "[Skill: {}] Executed with input: {}",
            self.name, input
        ))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_skill_registry() {
        let mut registry = SkillRegistry::new();
        assert_eq!(registry.list_names().len(), 0);
    }
}
