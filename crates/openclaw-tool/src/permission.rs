//! # 权限检查
//!
//! 检查工具调用权限。

use openclaw_core::{Result, ExecutionContext, PermissionLevel};

/// 权限检查器
#[derive(Debug)]
pub struct PermissionChecker;

impl PermissionChecker {
    pub fn new() -> Self {
        Self
    }

    pub fn check_permission(
        &self,
        required_level: PermissionLevel,
        context: &ExecutionContext,
    ) -> Result<()> {
        match required_level {
            PermissionLevel::Public => Ok(()),
            PermissionLevel::Authenticated => {
                if context.user_id.is_some() || context.device_id.is_some() {
                    Ok(())
                } else {
                    Err(openclaw_core::OpenClawError::AuthFailed("Authentication required".to_string()))
                }
            }
            PermissionLevel::Trusted => {
                if context.scopes.iter().any(|s| s == "trusted") {
                    Ok(())
                } else {
                    Err(openclaw_core::OpenClawError::PermissionDenied(
                        openclaw_core::ToolId("unknown".to_string()),
                        PermissionLevel::Trusted,
                    ))
                }
            }
            PermissionLevel::Admin => {
                if context.scopes.iter().any(|s| s == "admin") {
                    Ok(())
                } else {
                    Err(openclaw_core::OpenClawError::PermissionDenied(
                        openclaw_core::ToolId("unknown".to_string()),
                        PermissionLevel::Admin,
                    ))
                }
            }
        }
    }
}

impl Default for PermissionChecker {
    fn default() -> Self {
        Self::new()
    }
}
