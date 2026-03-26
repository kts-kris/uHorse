//! 浏览器执行器
//!
//! 支持 headless 浏览器操作

use crate::error::{NodeError, NodeResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[cfg(feature = "browser")]
use std::sync::Arc;
#[cfg(feature = "browser")]
use std::time::Duration;
#[cfg(feature = "browser")]
use tokio::sync::Mutex;
#[cfg(feature = "browser")]
use tracing::debug;
use tracing::info;
use uhorse_protocol::{BrowserCommand, CommandOutput};

/// 浏览器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    /// 是否无头模式
    pub headless: bool,
    /// 窗口宽度
    pub width: u32,
    /// 窗口高度
    pub height: u32,
    /// 用户代理
    pub user_agent: Option<String>,
    /// 截图保存目录
    pub screenshot_dir: PathBuf,
    /// 默认超时
    pub default_timeout_secs: u64,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            headless: true,
            width: 1920,
            height: 1080,
            user_agent: None,
            screenshot_dir: PathBuf::from("/tmp/uhorse-browser"),
            default_timeout_secs: 30,
        }
    }
}

/// 浏览器执行器
pub struct BrowserExecutor {
    /// 配置
    config: BrowserConfig,
    /// 当前会话状态
    #[cfg(feature = "browser")]
    session: Arc<Mutex<Option<BrowserSession>>>,
}

#[cfg(feature = "browser")]
struct BrowserSession {
    browser: headless_chrome::Browser,
    tab: Arc<headless_chrome::Tab>,
    current_url: String,
}

impl BrowserExecutor {
    /// 创建新的浏览器执行器
    pub fn new(config: BrowserConfig) -> Self {
        // 确保截图目录存在
        std::fs::create_dir_all(&config.screenshot_dir).ok();

        Self {
            config,
            #[cfg(feature = "browser")]
            session: Arc::new(Mutex::new(None)),
        }
    }

    /// 使用默认配置创建
    pub fn default_executor() -> Self {
        Self::new(BrowserConfig::default())
    }

    /// 执行浏览器命令
    pub async fn execute(&self, cmd: &BrowserCommand) -> NodeResult<CommandOutput> {
        let action = format!("{:?}", cmd);
        info!("Executing browser command: {}", action);

        match cmd {
            BrowserCommand::Navigate { url } => self.navigate(url).await,
            BrowserCommand::Screenshot {
                selector,
                full_page,
            } => self.screenshot(selector.as_deref(), *full_page).await,
            BrowserCommand::Click { selector } => self.click(selector).await,
            BrowserCommand::Type { selector, text } => self.type_text(selector, text).await,
            BrowserCommand::WaitFor {
                selector,
                timeout_secs,
            } => self.wait_for(selector, *timeout_secs).await,
            BrowserCommand::GetText { selector } => self.get_text(selector).await,
            BrowserCommand::Evaluate { script } => self.evaluate(script).await,
            BrowserCommand::Close => self.close().await,
        }
    }

    /// 导航到 URL
    #[cfg(feature = "browser")]
    async fn navigate(&self, url: &str) -> NodeResult<CommandOutput> {
        use headless_chrome::{Browser, LaunchOptionsBuilder};

        debug!("Navigating to: {}", url);

        let mut session = self.session.lock().await;

        // 如果没有现有会话，创建新的
        if session.is_none() {
            let browser = Browser::new(
                LaunchOptionsBuilder::default()
                    .headless(self.config.headless)
                    .window_size(Some((self.config.width, self.config.height)))
                    .build()
                    .map_err(|e| {
                        NodeError::Execution(format!("Failed to launch browser: {}", e))
                    })?,
            )
            .map_err(|e| NodeError::Execution(format!("Failed to create browser: {}", e)))?;

            let tab = browser
                .new_tab()
                .map_err(|e| NodeError::Execution(format!("Failed to create tab: {}", e)))?;

            *session = Some(BrowserSession {
                browser,
                tab: Arc::new(tab),
                current_url: String::new(),
            });
        }

        let session = session.as_mut().unwrap();
        let tab = &session.tab;

        tab.navigate_to(url)
            .map_err(|e| NodeError::Execution(format!("Navigation failed: {}", e)))?;

        // 等待页面加载
        tab.wait_until_navigated()
            .map_err(|e| NodeError::Execution(format!("Wait for navigation failed: {}", e)))?;

        session.current_url = url.to_string();

        Ok(CommandOutput::json(serde_json::json!({
            "url": url,
            "status": "loaded"
        })))
    }

    /// 导航到 URL (未启用 feature)
    #[cfg(not(feature = "browser"))]
    async fn navigate(&self, _url: &str) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }

    /// 截图
    #[cfg(feature = "browser")]
    async fn screenshot(
        &self,
        _selector: Option<&str>,
        _full_page: bool,
    ) -> NodeResult<CommandOutput> {
        // TODO: headless_chrome 1.0 API changed - need to update for new capture_screenshot signature
        // The new API requires different parameters. This needs to be fixed.
        Err(NodeError::Execution(
            "Screenshot feature temporarily disabled due to headless_chrome API changes"
                .to_string(),
        ))
    }

    /// 截图 (未启用 feature)
    #[cfg(not(feature = "browser"))]
    async fn screenshot(
        &self,
        _selector: Option<&str>,
        _full_page: bool,
    ) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }

    /// 点击元素
    #[cfg(feature = "browser")]
    async fn click(&self, selector: &str) -> NodeResult<CommandOutput> {
        debug!("Clicking element: {}", selector);

        let session = self.session.lock().await;
        let session = session
            .as_ref()
            .ok_or_else(|| NodeError::Execution("No active browser session".to_string()))?;

        let tab = &session.tab;

        // 使用选择器查找元素并点击
        let element = tab.find_element(selector).map_err(|e| {
            NodeError::Execution(format!("Element not found '{}': {}", selector, e))
        })?;

        element
            .click()
            .map_err(|e| NodeError::Execution(format!("Click failed: {}", e)))?;

        Ok(CommandOutput::text("Clicked successfully"))
    }

    /// 点击元素 (未启用 feature)
    #[cfg(not(feature = "browser"))]
    async fn click(&self, _selector: &str) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }

    /// 输入文本
    #[cfg(feature = "browser")]
    async fn type_text(&self, selector: &str, text: &str) -> NodeResult<CommandOutput> {
        debug!("Typing into element: {}", selector);

        let session = self.session.lock().await;
        let session = session
            .as_ref()
            .ok_or_else(|| NodeError::Execution("No active browser session".to_string()))?;

        let tab = &session.tab;

        let element = tab.find_element(selector).map_err(|e| {
            NodeError::Execution(format!("Element not found '{}': {}", selector, e))
        })?;

        // 先点击聚焦
        element
            .click()
            .map_err(|e| NodeError::Execution(format!("Click failed: {}", e)))?;

        // 输入文本
        element
            .type_into(text)
            .map_err(|e| NodeError::Execution(format!("Type failed: {}", e)))?;

        Ok(CommandOutput::text("Typed successfully"))
    }

    /// 输入文本 (未启用 feature)
    #[cfg(not(feature = "browser"))]
    async fn type_text(&self, _selector: &str, _text: &str) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }

    /// 等待元素
    #[cfg(feature = "browser")]
    async fn wait_for(&self, selector: &str, timeout_secs: u64) -> NodeResult<CommandOutput> {
        debug!("Waiting for element: {}", selector);

        let session = self.session.lock().await;
        let session = session
            .as_ref()
            .ok_or_else(|| NodeError::Execution("No active browser session".to_string()))?;

        let tab = &session.tab;

        // 使用 tokio 超时
        let timeout = Duration::from_secs(timeout_secs);
        let start = std::time::Instant::now();

        loop {
            if tab.find_element(selector).is_ok() {
                return Ok(CommandOutput::text("Element found"));
            }

            if start.elapsed() > timeout {
                return Err(NodeError::Execution(format!(
                    "Timeout waiting for element: {}",
                    selector
                )));
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// 等待元素 (未启用 feature)
    #[cfg(not(feature = "browser"))]
    async fn wait_for(&self, _selector: &str, _timeout_secs: u64) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }

    /// 获取文本
    #[cfg(feature = "browser")]
    async fn get_text(&self, selector: &str) -> NodeResult<CommandOutput> {
        debug!("Getting text from element: {}", selector);

        let session = self.session.lock().await;
        let session = session
            .as_ref()
            .ok_or_else(|| NodeError::Execution("No active browser session".to_string()))?;

        let tab = &session.tab;

        let element = tab.find_element(selector).map_err(|e| {
            NodeError::Execution(format!("Element not found '{}': {}", selector, e))
        })?;

        // 使用 JavaScript 获取文本内容
        let text = tab
            .evaluate(
                &format!(
                    "document.querySelector('{}').textContent",
                    selector.replace('\'', "\\'")
                ),
                false,
            )
            .map_err(|e| NodeError::Execution(format!("Get text failed: {}", e)))?
            .value
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        Ok(CommandOutput::text(text))
    }

    /// 获取文本 (未启用 feature)
    #[cfg(not(feature = "browser"))]
    async fn get_text(&self, _selector: &str) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }

    /// 执行 JavaScript
    #[cfg(feature = "browser")]
    async fn evaluate(&self, script: &str) -> NodeResult<CommandOutput> {
        debug!(
            "Evaluating JavaScript: {}...",
            &script[..script.len().min(50)]
        );

        let session = self.session.lock().await;
        let session = session
            .as_ref()
            .ok_or_else(|| NodeError::Execution("No active browser session".to_string()))?;

        let tab = &session.tab;

        let result = tab
            .evaluate(script, false)
            .map_err(|e| NodeError::Execution(format!("Script execution failed: {}", e)))?;

        let value = result.value.unwrap_or(serde_json::Value::Null);

        Ok(CommandOutput::json(serde_json::json!({
            "result": value
        })))
    }

    /// 执行 JavaScript (未启用 feature)
    #[cfg(not(feature = "browser"))]
    async fn evaluate(&self, _script: &str) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }

    /// 关闭浏览器
    #[cfg(feature = "browser")]
    async fn close(&self) -> NodeResult<CommandOutput> {
        debug!("Closing browser");

        let mut session = self.session.lock().await;
        if let Some(s) = session.take() {
            // Browser will be closed automatically when dropped
            drop(s.tab);
            drop(s.browser);
        }

        Ok(CommandOutput::text("Browser closed"))
    }

    /// 关闭浏览器 (未启用 feature)
    #[cfg(not(feature = "browser"))]
    async fn close(&self) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }
}

impl std::fmt::Debug for BrowserExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserExecutor")
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = BrowserConfig::default();
        assert!(config.headless);
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
    }
}
