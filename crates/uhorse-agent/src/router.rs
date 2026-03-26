//! # Router - 消息路由器
//!
//! 负责将消息路由到正确的 Agent。

use crate::error::AgentResult;

/// 路由目标
#[derive(Debug, Clone)]
pub enum RouteTarget {
    /// 指定 Agent
    Agent(String),
    /// 技能
    Skill(String),
    /// 多个 Agent（并行）
    Multiple(Vec<String>),
    /// 条件路由
    Conditional {
        /// 条件表达式。
        condition: String,
        /// 条件成立时的目标。
        then: String,
        /// 条件不成立时的目标。
        r#else: String,
    },
}

/// 路由规则
#[derive(Debug, Clone)]
pub struct Route {
    /// 路由名称
    pub name: String,
    /// 匹配模式
    pub pattern: RoutePattern,
    /// 目标
    pub target: RouteTarget,
}

/// 路由模式
#[derive(Debug, Clone)]
pub enum RoutePattern {
    /// 前缀匹配
    Prefix(String),
    /// 关键词匹配
    Keyword(String),
    /// 正则表达式
    Regex(String),
    /// 意图匹配（通过 LLM）
    Intent(String),
    /// 默认路由
    Default,
}

/// 路由器
#[derive(Debug, Clone)]
pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    /// 创建新的路由器
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// 添加路由
    pub fn add_route(&mut self, route: Route) {
        self.routes.push(route);
    }

    /// 路由消息
    pub fn route(&self, message: &str) -> AgentResult<RouteTarget> {
        for route in &self.routes {
            match &route.pattern {
                RoutePattern::Prefix(prefix) => {
                    if message.starts_with(prefix) {
                        return Ok(route.target.clone());
                    }
                }
                RoutePattern::Keyword(keyword) => {
                    if message.contains(keyword) {
                        return Ok(route.target.clone());
                    }
                }
                RoutePattern::Default => {
                    return Ok(route.target.clone());
                }
                _ => {
                    // 其他模式需要更复杂的处理
                    continue;
                }
            }
        }

        // 默认返回
        Ok(RouteTarget::Agent("default".to_string()))
    }

    /// 根据意图路由
    pub async fn route_by_intent(
        &self,
        message: &str,
        _llm: Option<&dyn uhorse_llm::LLMClient>,
    ) -> AgentResult<RouteTarget> {
        // 简化实现：检查是否有意图匹配的路由
        for route in &self.routes {
            if let RoutePattern::Intent(intent) = &route.pattern {
                // 这里应该调用 LLM 来检测意图
                // 简化实现：检查消息是否包含意图关键词
                if message.contains(intent) {
                    return Ok(route.target.clone());
                }
            }
        }

        Ok(RouteTarget::Agent("default".to_string()))
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}
