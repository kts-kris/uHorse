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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

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
    #[serde(default)]
    pub parameters: Vec<SkillParameter>,
    /// 所需权限
    #[serde(default)]
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
    /// 执行命令
    #[serde(default)]
    pub executable: Option<String>,
    /// 命令参数
    #[serde(default)]
    pub args: Vec<String>,
    /// 环境变量
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl Default for SkillConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout: default_timeout(),
            max_retries: default_max_retries(),
            executable: None,
            args: Vec::new(),
            env: HashMap::new(),
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

    /// 判断是否为空。
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
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

    /// 从目录创建注册表。
    pub async fn from_dir(dir: PathBuf) -> AgentResult<Self> {
        let mut registry = Self::new();
        if dir.exists() {
            let _ = registry.load_from_dir(dir).await?;
        }
        Ok(registry)
    }

    /// 从目录加载单个技能
    async fn load_skill(dir: &Path) -> AgentResult<Skill> {
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

        let executor: Arc<dyn SkillExecutor> = if let Some(executable) = config.executable.clone() {
            Arc::new(ProcessSkillExecutor {
                name: manifest.name.clone(),
                description: manifest.description.clone(),
                executable,
                args: config.args.clone(),
                env: config.env.clone(),
                timeout: Duration::from_secs(config.timeout),
            })
        } else {
            Arc::new(DummySkillExecutor {
                name: manifest.name.clone(),
                description: manifest.description.clone(),
            })
        };

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

/// 分层技能注册表。
#[derive(Clone, Default)]
pub struct LayeredSkillRegistry {
    global: SkillRegistry,
    tenant: HashMap<String, SkillRegistry>,
    user: HashMap<String, SkillRegistry>,
}

impl LayeredSkillRegistry {
    /// 创建新的分层注册表。
    pub fn new(global: SkillRegistry) -> Self {
        Self {
            global,
            tenant: HashMap::new(),
            user: HashMap::new(),
        }
    }

    /// 注册 tenant 级技能表。
    pub fn register_tenant_registry(&mut self, scope: impl Into<String>, registry: SkillRegistry) {
        self.tenant.insert(scope.into(), registry);
    }

    /// 注册 user 级技能表。
    pub fn register_user_registry(&mut self, scope: impl Into<String>, registry: SkillRegistry) {
        self.user.insert(scope.into(), registry);
    }

    /// 按 `user > tenant > global` 解析技能。
    pub fn get_for_scopes(&self, scopes: &[String], name: &str) -> Option<Skill> {
        for scope in scopes {
            if let Some(skill) = self
                .user
                .get(scope)
                .and_then(|registry| registry.get(name))
                .or_else(|| {
                    self.tenant
                        .get(scope)
                        .and_then(|registry| registry.get(name))
                })
            {
                return Some(skill);
            }
        }

        self.global.get(name)
    }

    /// 按 `user > tenant > global` 列出可见技能。
    pub fn list_names_for_scopes(&self, scopes: &[String]) -> Vec<String> {
        let mut names = self.global.list_names();
        for scope in scopes.iter().rev() {
            if let Some(registry) = self.tenant.get(scope) {
                names.extend(registry.list_names());
            }
            if let Some(registry) = self.user.get(scope) {
                names.extend(registry.list_names());
            }
        }
        names.sort();
        names.dedup();
        names
    }

    /// 返回技能来源层级。
    pub fn source_for_scopes(&self, scopes: &[String], name: &str) -> Option<&'static str> {
        for scope in scopes {
            if self
                .user
                .get(scope)
                .and_then(|registry| registry.get(name))
                .is_some()
            {
                return Some("user");
            }
            if self
                .tenant
                .get(scope)
                .and_then(|registry| registry.get(name))
                .is_some()
            {
                return Some("tenant");
            }
        }
        self.global.get(name).map(|_| "global")
    }

    /// 列出所有层级中的技能名称。
    pub fn list_all_names(&self) -> Vec<String> {
        let mut names = self.global.list_names();
        for registry in self.tenant.values() {
            names.extend(registry.list_names());
        }
        for registry in self.user.values() {
            names.extend(registry.list_names());
        }
        names.sort();
        names.dedup();
        names
    }

    /// 返回任意层级中的技能定义，优先 user / tenant / global。
    pub fn get_any(&self, name: &str) -> Option<Skill> {
        for registry in self.user.values() {
            if let Some(skill) = registry.get(name) {
                return Some(skill);
            }
        }
        for registry in self.tenant.values() {
            if let Some(skill) = registry.get(name) {
                return Some(skill);
            }
        }
        self.global.get(name)
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

#[derive(Clone)]
struct ProcessSkillExecutor {
    name: String,
    description: String,
    executable: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    timeout: Duration,
}

#[async_trait::async_trait]
impl SkillExecutor for ProcessSkillExecutor {
    async fn execute(&self, input: &str) -> AgentResult<String> {
        let mut command = tokio::process::Command::new(&self.executable);
        command.args(&self.args);
        command.env("SKILL_INPUT", input);
        command.env("SKILL_NAME", &self.name);
        for (key, value) in &self.env {
            command.env(key, value);
        }

        let output = tokio::time::timeout(self.timeout, command.output())
            .await
            .map_err(|_| AgentError::SkillExecution {
                skill: self.name.clone(),
                error: format!("timed out after {}s", self.timeout.as_secs()),
            })?
            .map_err(|error| AgentError::SkillExecution {
                skill: self.name.clone(),
                error: error.to_string(),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if !output.status.success() {
            let error = if stderr.is_empty() {
                format!("process exited with status {}", output.status)
            } else {
                stderr
            };
            return Err(AgentError::SkillExecution {
                skill: self.name.clone(),
                error,
            });
        }

        if !stderr.is_empty() {
            return Err(AgentError::SkillExecution {
                skill: self.name.clone(),
                error: stderr,
            });
        }

        if stdout.is_empty() {
            return Ok(String::new());
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
            return Ok(serde_json::to_string_pretty(&json).unwrap_or(stdout));
        }

        Ok(stdout)
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
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_skill_registry() {
        let registry = SkillRegistry::new();
        assert_eq!(registry.list_names().len(), 0);
    }

    #[tokio::test]
    async fn test_load_from_dir_uses_process_executor() {
        let tempdir = tempdir().unwrap();
        let skill_dir = tempdir.path().join("echo-skill");
        tokio::fs::create_dir_all(&skill_dir).await.unwrap();
        tokio::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: echo
version: 1.0.0
description: echo skill
author: test
parameters: []
permissions: []
---
"#,
        )
        .await
        .unwrap();
        tokio::fs::write(
            skill_dir.join("skill.toml"),
            r#"enabled = true
timeout = 5
executable = "python3"
args = ["-c", "import os; print(os.environ['SKILL_INPUT'])"]
"#,
        )
        .await
        .unwrap();

        let mut registry = SkillRegistry::new();
        let count = registry
            .load_from_dir(tempdir.path().to_path_buf())
            .await
            .unwrap();
        assert_eq!(count, 1);

        let skill = registry.get("echo").unwrap();
        let output = skill.execute("hello").await.unwrap();
        assert_eq!(output, "hello");
    }

    #[tokio::test]
    async fn test_layered_skill_registry_resolves_user_before_tenant_before_global() {
        let tempdir = tempdir().unwrap();

        async fn write_skill(root: &std::path::Path, name: &str, command: &str) {
            let skill_dir = root.join(name);
            tokio::fs::create_dir_all(&skill_dir).await.unwrap();
            tokio::fs::write(
                skill_dir.join("SKILL.md"),
                format!(
                    "---\nname: {}\nversion: 1.0.0\ndescription: {} skill\nauthor: test\nparameters: []\npermissions: []\n---\n",
                    name, name
                ),
            )
            .await
            .unwrap();
            tokio::fs::write(
                skill_dir.join("skill.toml"),
                format!(
                    "enabled = true\ntimeout = 5\nexecutable = \"python3\"\nargs = [\"-c\", \"{}\"]\n",
                    command
                ),
            )
            .await
            .unwrap();
        }

        let global_dir = tempdir.path().join("global");
        let tenant_dir = tempdir.path().join("tenant");
        let user_dir = tempdir.path().join("user");
        tokio::fs::create_dir_all(&global_dir).await.unwrap();
        tokio::fs::create_dir_all(&tenant_dir).await.unwrap();
        tokio::fs::create_dir_all(&user_dir).await.unwrap();

        write_skill(&global_dir, "echo", "print('global')").await;
        write_skill(&tenant_dir, "echo", "print('tenant')").await;
        write_skill(&user_dir, "echo", "print('user')").await;

        let global = SkillRegistry::from_dir(global_dir).await.unwrap();
        let tenant = SkillRegistry::from_dir(tenant_dir).await.unwrap();
        let user = SkillRegistry::from_dir(user_dir).await.unwrap();

        let mut layered = LayeredSkillRegistry::new(global);
        layered.register_tenant_registry("tenant:dingtalk:corp-1", tenant);
        layered.register_user_registry("user:dingtalk:user-1", user);

        let scopes = vec![
            "user:dingtalk:user-1".to_string(),
            "tenant:dingtalk:corp-1".to_string(),
            "global".to_string(),
        ];

        let output = layered
            .get_for_scopes(&scopes, "echo")
            .unwrap()
            .execute("ignored")
            .await
            .unwrap();
        assert_eq!(output, "user");
        assert_eq!(layered.source_for_scopes(&scopes, "echo"), Some("user"));
        assert_eq!(
            layered.list_names_for_scopes(&scopes),
            vec!["echo".to_string()]
        );
    }
}
