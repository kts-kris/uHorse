//! # 工具注册表
//!
//! 管理工具的注册和调用。

use openclaw_core::{ToolRegistry, ToolExecutor, ToolId, ExecutionContext, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 工具注册表实现
#[derive(Debug)]
pub struct ToolRegistryImpl {
    tools: Arc<RwLock<HashMap<ToolId, Arc<dyn ToolExecutor>>>>,
}

impl ToolRegistryImpl {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for ToolRegistryImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ToolRegistry for ToolRegistryImpl {
    async fn register_tool(&mut self, tool: Box<dyn ToolExecutor>) -> Result<()> {
        let id = tool.id().clone();
        self.tools.write().await.insert(id, Arc::from(tool));
        Ok(())
    }

    async fn unregister_tool(&mut self, id: &ToolId) -> Result<()> {
        self.tools.write().await.remove(id);
        Ok(())
    }

    async fn get_tool(&self, id: &ToolId) -> Result<Option<Box<dyn ToolExecutor>>> {
        // 注意：这里无法返回 Arc 的克隆，因为 trait 不兼容
        // 实际使用时需要重新设计
        Ok(None)
    }

    async fn list_tools(&self) -> Result<Vec<Box<dyn ToolExecutor>>> {
        Ok(Vec::new())
    }

    async fn call_tool(&self, id: &ToolId, params: serde_json::Value, context: &ExecutionContext) -> Result<serde_json::Value> {
        let tools = self.tools.read().await;
        let tool = tools.get(id)
            .ok_or_else(|| openclaw_core::OpenClawError::ToolNotFound(id.clone()))?;

        tool.execute(params, context).await
    }
}
