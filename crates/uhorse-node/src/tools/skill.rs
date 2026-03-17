//! 技能执行器
//!
//! 复用 uhorse-agent 技能系统在 Node 端执行技能

use crate::error::{NodeError, NodeResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uhorse_protocol::{CommandOutput, SkillCommand};

/// 技能定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    /// 技能名称
    pub name: String,
    /// 版本
    pub version: String,
    /// 描述
    pub description: String,
    /// 执行命令
    pub executable: String,
    /// 参数
    pub args: Vec<String>,
    /// 环境变量
    pub env: HashMap<String, String>,
    /// 超时
    pub timeout_secs: u64,
    /// 输入 Schema
    pub input_schema: Option<serde_json::Value>,
    /// 输出 Schema
    pub output_schema: Option<serde_json::Value>,
}

/// 技能执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    /// 技能名称
    pub skill_name: String,
    /// 是否成功
    pub success: bool,
    /// 输出
    pub output: serde_json::Value,
    /// 错误
    pub error: Option<String>,
    /// 执行时间 (ms)
    pub duration_ms: u64,
}

/// 技能执行器
pub struct SkillExecutor {
    /// 技能注册表
    skills: Arc<RwLock<HashMap<String, SkillDefinition>>>,
    /// 技能目录
    skill_dirs: Vec<PathBuf>,
    /// 默认超时
    default_timeout: Duration,
}

impl SkillExecutor {
    /// 创建新的技能执行器
    pub fn new() -> Self {
        let mut skill_dirs = Vec::new();

        // 添加默认技能目录
        if let Some(home) = std::env::var_os("HOME") {
            let home_path = PathBuf::from(home);
            skill_dirs.push(home_path.join(".uhorse").join("skills"));
        }

        Self {
            skills: Arc::new(RwLock::new(HashMap::new())),
            skill_dirs,
            default_timeout: Duration::from_secs(60),
        }
    }

    /// 添加技能目录
    pub fn add_skill_dir(&mut self, dir: PathBuf) {
        self.skill_dirs.push(dir);
    }

    /// 注册技能
    pub async fn register_skill(&self, skill: SkillDefinition) -> NodeResult<()> {
        let mut skills = self.skills.write().await;
        let key = if skill.version.is_empty() {
            skill.name.clone()
        } else {
            format!("{}@{}", skill.name, skill.version)
        };

        info!("Registering skill: {}", key);
        skills.insert(key, skill);
        Ok(())
    }

    /// 注销技能
    pub async fn unregister_skill(&self, name: &str) -> NodeResult<()> {
        let mut skills = self.skills.write().await;
        if skills.remove(name).is_some() {
            info!("Unregistered skill: {}", name);
        }
        Ok(())
    }

    /// 获取技能
    pub async fn get_skill(&self, name: &str) -> Option<SkillDefinition> {
        let skills = self.skills.read().await;

        // 尝试精确匹配
        if let Some(skill) = skills.get(name) {
            return Some(skill.clone());
        }

        // 尝试不带版本匹配
        for (key, skill) in skills.iter() {
            if key.starts_with(&format!("{}@", name)) || key == name {
                return Some(skill.clone());
            }
        }

        None
    }

    /// 列出所有技能
    pub async fn list_skills(&self) -> Vec<SkillDefinition> {
        let skills = self.skills.read().await;
        skills.values().cloned().collect()
    }

    /// 执行技能
    pub async fn execute(&self, cmd: &SkillCommand) -> NodeResult<CommandOutput> {
        info!("Executing skill: {} (version: {:?})", cmd.skill_name, cmd.version);

        // 查找技能
        let skill = self.find_skill(&cmd.skill_name, cmd.version.as_deref()).await?;

        // 验证输入
        if let Some(schema) = &skill.input_schema {
            self.validate_input(&cmd.input, schema)?;
        }

        // 执行技能
        let start = Instant::now();
        let result = self.execute_skill(&skill, &cmd.input, cmd.timeout).await?;
        let duration = start.elapsed();

        // 验证输出
        if let Some(schema) = &skill.output_schema {
            if let Some(ref output) = result.output.as_object() {
                self.validate_output(output, schema)?;
            }
        }

        Ok(CommandOutput::json(serde_json::json!({
            "skill_name": skill.name,
            "success": result.success,
            "output": result.output,
            "error": result.error,
            "duration_ms": duration.as_millis() as u64
        })))
    }

    /// 查找技能
    async fn find_skill(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> NodeResult<SkillDefinition> {
        let skills = self.skills.read().await;

        // 如果指定了版本，精确匹配
        if let Some(v) = version {
            let key = format!("{}@{}", name, v);
            if let Some(skill) = skills.get(&key) {
                return Ok(skill.clone());
            }
        }

        // 尝试不带版本匹配
        if let Some(skill) = skills.get(name) {
            return Ok(skill.clone());
        }

        // 查找任意版本
        for (key, skill) in skills.iter() {
            if key.starts_with(&format!("{}@", name)) {
                return Ok(skill.clone());
            }
        }

        Err(NodeError::Execution(format!(
            "Skill '{}' not found",
            name
        )))
    }

    /// 验证输入
    fn validate_input(
        &self,
        input: &serde_json::Value,
        schema: &serde_json::Value,
    ) -> NodeResult<()> {
        // 简单的 JSON Schema 验证
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            if let Some(properties) = input.as_object() {
                for field in required {
                    if let Some(field_name) = field.as_str() {
                        if !properties.contains_key(field_name) {
                            return Err(NodeError::Execution(format!(
                                "Missing required field: {}",
                                field_name
                            )));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 验证输出
    fn validate_output(
        &self,
        _output: &serde_json::Map<String, serde_json::Value>,
        _schema: &serde_json::Value,
    ) -> NodeResult<()> {
        // 简单实现，不做严格验证
        Ok(())
    }

    /// 执行技能命令
    async fn execute_skill(
        &self,
        skill: &SkillDefinition,
        input: &serde_json::Value,
        timeout: Duration,
    ) -> NodeResult<SkillResult> {
        debug!("Executing skill command: {}", skill.executable);

        let input_json = serde_json::to_string(input)?;

        // 构建命令
        let mut cmd = tokio::process::Command::new(&skill.executable);
        cmd.args(&skill.args)
            .env("SKILL_INPUT", &input_json)
            .env("SKILL_NAME", &skill.name);

        // 添加环境变量
        for (key, value) in &skill.env {
            cmd.env(key, value);
        }

        // 设置超时
        let timeout_duration = if timeout.is_zero() {
            Duration::from_secs(skill.timeout_secs)
        } else {
            timeout
        };

        // 执行命令
        let output = tokio::time::timeout(timeout_duration, cmd.output())
            .await
            .map_err(|_| NodeError::Execution(format!("Skill '{}' timed out", skill.name)))?
            .map_err(|e| NodeError::Execution(format!("Failed to execute skill: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            // 解析输出
            let output_value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
                serde_json::json!({ "raw_output": stdout })
            });

            Ok(SkillResult {
                skill_name: skill.name.clone(),
                success: true,
                output: output_value,
                error: None,
                duration_ms: 0, // 由调用者设置
            })
        } else {
            Ok(SkillResult {
                skill_name: skill.name.clone(),
                success: false,
                output: serde_json::Value::Null,
                error: Some(stderr),
                duration_ms: 0,
            })
        }
    }

    /// 从目录加载技能
    pub async fn load_skills_from_dir(&self, dir: &PathBuf) -> NodeResult<usize> {
        if !dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        let entries = std::fs::read_dir(dir).map_err(|e| {
            NodeError::Execution(format!("Failed to read skill directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                NodeError::Execution(format!("Failed to read entry: {}", e))
            })?;

            let path = entry.path();
            if path.is_dir() {
                // 查找 SKILL.md 或 skill.json
                let skill_md = path.join("SKILL.md");
                let skill_json = path.join("skill.json");

                if skill_json.exists() {
                    if let Ok(content) = std::fs::read_to_string(&skill_json) {
                        if let Ok(skill) = serde_json::from_str::<SkillDefinition>(&content) {
                            self.register_skill(skill).await?;
                            count += 1;
                        }
                    }
                } else if skill_md.exists() {
                    // 从 SKILL.md 解析技能定义
                    if let Some(skill) = self.parse_skill_md(&skill_md).await? {
                        self.register_skill(skill).await?;
                        count += 1;
                    }
                }
            }
        }

        info!("Loaded {} skills from {:?}", count, dir);
        Ok(count)
    }

    /// 从 SKILL.md 解析技能定义
    async fn parse_skill_md(&self, path: &PathBuf) -> NodeResult<Option<SkillDefinition>> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            NodeError::Execution(format!("Failed to read SKILL.md: {}", e))
        })?;

        // 简单解析 SKILL.md 格式
        let mut name = String::new();
        let mut version = String::from("1.0.0");
        let mut description = String::new();
        let mut executable = String::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("# ") {
                name = line[2..].to_string();
            } else if line.starts_with("## Version") {
                // 下一行是版本
            } else if line.starts_with("## Description") {
                // 下一行是描述
            } else if line.starts_with("```") {
                // 代码块
            }
        }

        if name.is_empty() {
            return Ok(None);
        }

        // 尝试推断可执行文件
        let skill_dir = path.parent().unwrap();
        if skill_dir.join("run.sh").exists() {
            executable = skill_dir.join("run.sh").to_string_lossy().to_string();
        } else if skill_dir.join("run.py").exists() {
            executable = "python3".to_string();
        }

        Ok(Some(SkillDefinition {
            name,
            version,
            description,
            executable,
            args: vec![],
            env: HashMap::new(),
            timeout_secs: 60,
            input_schema: None,
            output_schema: None,
        }))
    }
}

impl Default for SkillExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SkillExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillExecutor")
            .field("skill_dirs", &self.skill_dirs)
            .field("default_timeout", &self.default_timeout)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_skill() {
        let executor = SkillExecutor::new();
        let skill = SkillDefinition {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "Test skill".to_string(),
            executable: "echo".to_string(),
            args: vec![],
            env: HashMap::new(),
            timeout_secs: 30,
            input_schema: None,
            output_schema: None,
        };

        executor.register_skill(skill).await.unwrap();
        let skills = executor.list_skills().await;
        assert_eq!(skills.len(), 1);
    }
}
