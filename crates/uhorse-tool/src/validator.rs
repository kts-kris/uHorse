//! # 参数验证器
//!
//! 使用 JSON Schema 验证工具参数。

use uhorse_core::Result;

/// 参数验证器
#[derive(Debug)]
pub struct ToolValidator {
    validator: jsonschema::Validator,
}

impl ToolValidator {
    pub fn new(schema: &serde_json::Value) -> Result<Self> {
        let validator = jsonschema::Validator::new(schema)
            .map_err(|e| uhorse_core::UHorseError::ToolValidationFailed(e.to_string()))?;
        Ok(Self { validator })
    }

    pub fn validate(&self, params: &serde_json::Value) -> Result<()> {
        use std::fmt::Write;

        let result = self.validator.validate(params);
        if let Err(errors) = result {
            let mut msg = String::from("Validation failed:");
            for error in errors {
                let _ = write!(msg, " {}", error);
            }
            return Err(uhorse_core::UHorseError::ToolValidationFailed(msg));
        }
        Ok(())
    }
}
