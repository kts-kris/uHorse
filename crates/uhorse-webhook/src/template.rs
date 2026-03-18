//! Template System
//!
//! 自定义 Webhook 载荷模板

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Webhook 模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookTemplate {
    /// 模板 ID
    pub id: String,
    /// 模板名称
    pub name: String,
    /// 描述
    pub description: Option<String>,
    /// 模板内容
    pub content: String,
    /// 内容类型
    pub content_type: ContentType,
    /// 变量定义
    #[serde(default)]
    pub variables: HashMap<String, VariableDefinition>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

impl WebhookTemplate {
    /// 创建新模板
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            description: None,
            content: content.into(),
            content_type: ContentType::Json,
            variables: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// 设置描述
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 设置内容类型
    pub fn with_content_type(mut self, content_type: ContentType) -> Self {
        self.content_type = content_type;
        self
    }

    /// 添加变量
    pub fn add_variable(mut self, name: impl Into<String>, definition: VariableDefinition) -> Self {
        self.variables.insert(name.into(), definition);
        self
    }
}

/// 内容类型
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    /// JSON
    Json,
    /// Form URL Encoded
    FormUrlEncoded,
    /// XML
    Xml,
    /// Plain Text
    PlainText,
}

/// 变量定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDefinition {
    /// 变量类型
    pub var_type: VariableType,
    /// 是否必需
    #[serde(default)]
    pub required: bool,
    /// 默认值
    #[serde(default)]
    pub default: Option<Value>,
    /// 描述
    #[serde(default)]
    pub description: Option<String>,
}

impl VariableDefinition {
    /// 创建新变量定义
    pub fn new(var_type: VariableType) -> Self {
        Self {
            var_type,
            required: false,
            default: None,
            description: None,
        }
    }

    /// 设置为必需
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// 设置默认值
    pub fn with_default(mut self, default: Value) -> Self {
        self.default = Some(default);
        self
    }
}

/// 变量类型
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableType {
    String,
    Number,
    Boolean,
    Object,
    Array,
}

/// 模板引擎
pub struct TemplateEngine {
    /// 左分隔符
    left_delimiter: String,
    /// 右分隔符
    right_delimiter: String,
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateEngine {
    /// 创建新的模板引擎
    pub fn new() -> Self {
        Self {
            left_delimiter: "{{".to_string(),
            right_delimiter: "}}".to_string(),
        }
    }

    /// 设置分隔符
    pub fn with_delimiters(mut self, left: &str, right: &str) -> Self {
        self.left_delimiter = left.to_string();
        self.right_delimiter = right.to_string();
        self
    }

    /// 渲染模板
    pub fn render(
        &self,
        template: &WebhookTemplate,
        variables: &HashMap<String, Value>,
    ) -> Result<String, crate::WebhookError> {
        // 验证变量
        self.validate_variables(template, variables)?;

        // 渲染模板
        let mut result = template.content.clone();

        for (key, value) in variables {
            let placeholder = format!("{}{}{}", self.left_delimiter, key, self.right_delimiter);
            let value_str = self.value_to_string(value);
            result = result.replace(&placeholder, &value_str);
        }

        // 替换默认值
        for (key, definition) in &template.variables {
            if !variables.contains_key(key) {
                if let Some(ref default) = definition.default {
                    let placeholder =
                        format!("{}{}{}", self.left_delimiter, key, self.right_delimiter);
                    let value_str = self.value_to_string(default);
                    result = result.replace(&placeholder, &value_str);
                }
            }
        }

        Ok(result)
    }

    /// 验证变量
    fn validate_variables(
        &self,
        template: &WebhookTemplate,
        variables: &HashMap<String, Value>,
    ) -> Result<(), crate::WebhookError> {
        for (key, definition) in &template.variables {
            if definition.required && !variables.contains_key(key) && definition.default.is_none() {
                return Err(crate::WebhookError::TemplateError(format!(
                    "Missing required variable: {}",
                    key
                )));
            }

            // 类型检查
            if let Some(value) = variables.get(key) {
                if !self.check_type(value, definition.var_type) {
                    return Err(crate::WebhookError::TemplateError(format!(
                        "Type mismatch for variable '{}': expected {:?}",
                        key, definition.var_type
                    )));
                }
            }
        }

        Ok(())
    }

    /// 检查类型
    fn check_type(&self, value: &Value, expected: VariableType) -> bool {
        match expected {
            VariableType::String => value.is_string(),
            VariableType::Number => value.is_number(),
            VariableType::Boolean => value.is_boolean(),
            VariableType::Object => value.is_object(),
            VariableType::Array => value.is_array(),
        }
    }

    /// 将值转换为字符串
    fn value_to_string(&self, value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Object(_) | Value::Array(_) => serde_json::to_string(value).unwrap_or_default(),
            Value::Null => String::new(),
        }
    }

    /// 提取模板中的变量
    pub fn extract_variables(&self, template: &str) -> Vec<String> {
        let mut variables = Vec::new();
        let mut start = 0;

        while let Some(begin) = template[start..].find(&self.left_delimiter) {
            let after_left = start + begin + self.left_delimiter.len();
            if let Some(end) = template[after_left..].find(&self.right_delimiter) {
                let var_name = template[after_left..after_left + end].trim().to_string();
                if !var_name.is_empty() && !variables.contains(&var_name) {
                    variables.push(var_name);
                }
                start = after_left + end + self.right_delimiter.len();
            } else {
                break;
            }
        }

        variables
    }
}

/// 预定义模板
pub fn default_templates() -> Vec<WebhookTemplate> {
    vec![
        // 事件通知模板
        WebhookTemplate::new(
            "Event Notification",
            r#"{
  "event_type": "{{event_type}}",
  "timestamp": "{{timestamp}}",
  "data": {{data}}
}"#,
        )
        .with_description("通用事件通知模板")
        .add_variable(
            "event_type",
            VariableDefinition::new(VariableType::String).required(),
        )
        .add_variable(
            "timestamp",
            VariableDefinition::new(VariableType::String)
                .with_default(Value::String(Utc::now().to_rfc3339())),
        )
        .add_variable(
            "data",
            VariableDefinition::new(VariableType::Object).required(),
        ),
        // Slack 消息模板
        WebhookTemplate::new(
            "Slack Message",
            r#"{
  "text": "{{message}}",
  "username": "{{username}}",
  "icon_emoji": "{{icon}}",
  "channel": "{{channel}}"
}"#,
        )
        .with_description("Slack 消息格式")
        .add_variable(
            "message",
            VariableDefinition::new(VariableType::String).required(),
        )
        .add_variable(
            "username",
            VariableDefinition::new(VariableType::String)
                .with_default(Value::String("uHorse Bot".to_string())),
        )
        .add_variable(
            "icon",
            VariableDefinition::new(VariableType::String)
                .with_default(Value::String(":robot_face:".to_string())),
        )
        .add_variable("channel", VariableDefinition::new(VariableType::String)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_template_creation() {
        let template = WebhookTemplate::new("Test Template", "Hello {{name}}!")
            .with_description("A test template");

        assert_eq!(template.name, "Test Template");
        assert!(template.description.is_some());
    }

    #[test]
    fn test_template_engine_render() {
        let engine = TemplateEngine::new();
        let template = WebhookTemplate::new("Test", "Hello {{name}}, you are {{age}} years old!");

        let mut variables = HashMap::new();
        variables.insert("name".to_string(), json!("Alice"));
        variables.insert("age".to_string(), json!(30));

        let result = engine.render(&template, &variables).unwrap();
        assert_eq!(result, "Hello Alice, you are 30 years old!");
    }

    #[test]
    fn test_template_engine_extract_variables() {
        let engine = TemplateEngine::new();
        let template = "Hello {{name}}, {{greeting}} from {{place}}!";

        let vars = engine.extract_variables(template);
        assert_eq!(vars.len(), 3);
        assert!(vars.contains(&"name".to_string()));
        assert!(vars.contains(&"greeting".to_string()));
        assert!(vars.contains(&"place".to_string()));
    }

    #[test]
    fn test_template_required_variable() {
        let engine = TemplateEngine::new();
        let template = WebhookTemplate::new("Test", "Hello {{name}}!").add_variable(
            "name",
            VariableDefinition::new(VariableType::String).required(),
        );

        let variables = HashMap::new();
        let result = engine.render(&template, &variables);

        assert!(result.is_err());
    }

    #[test]
    fn test_template_default_value() {
        let engine = TemplateEngine::new();
        let template = WebhookTemplate::new("Test", "Hello {{name}}!").add_variable(
            "name",
            VariableDefinition::new(VariableType::String).with_default(json!("World")),
        );

        let variables = HashMap::new();
        let result = engine.render(&template, &variables).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_template_type_check() {
        let engine = TemplateEngine::new();
        let template = WebhookTemplate::new("Test", "Count: {{count}}")
            .add_variable("count", VariableDefinition::new(VariableType::Number));

        let mut variables = HashMap::new();
        variables.insert("count".to_string(), json!("not a number"));

        let result = engine.render(&template, &variables);
        assert!(result.is_err());
    }

    #[test]
    fn test_default_templates() {
        let templates = default_templates();
        assert!(!templates.is_empty());
        assert!(templates.iter().any(|t| t.name == "Event Notification"));
        assert!(templates.iter().any(|t| t.name == "Slack Message"));
    }
}
