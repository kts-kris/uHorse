//! 浏览器执行器。
//!
//! 负责在 Node Runtime 中执行 `BrowserCommand`，并维护跨命令共享的浏览器会话。

use crate::error::{NodeError, NodeResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[cfg(feature = "browser")]
use std::sync::Arc;
#[cfg(feature = "browser")]
use std::time::{Duration, Instant};
#[cfg(feature = "browser")]
use tokio::sync::Mutex;
#[cfg(feature = "browser")]
use tracing::debug;
use tracing::info;
use uhorse_protocol::{BrowserCommand, BrowserResult, CommandOutput};

/// 浏览器配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BrowserConfig {
    /// 是否启用无头模式。
    pub headless: bool,
    /// 窗口宽度。
    pub width: u32,
    /// 窗口高度。
    pub height: u32,
    /// 用户代理。
    pub user_agent: Option<String>,
    /// 截图目录。
    pub screenshot_dir: PathBuf,
    /// 默认超时秒数。
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

/// 浏览器执行器。
#[derive(Default)]
pub(crate) struct BrowserExecutor {
    /// 执行配置。
    config: BrowserConfig,
    /// 当前浏览器会话。
    #[cfg(feature = "browser")]
    session: Arc<Mutex<Option<BrowserSession>>>,
}

#[cfg(feature = "browser")]
struct BrowserSession {
    /// 保持浏览器实例存活。
    _browser: headless_chrome::Browser,
    /// 当前活动标签页。
    tab: Arc<headless_chrome::Tab>,
}

impl BrowserExecutor {
    fn open_url(url: &str) -> NodeResult<()> {
        #[cfg(test)]
        {
            let _ = url;
            Ok(())
        }

        #[cfg(not(test))]
        {
            open::that(url)
                .map_err(|e| NodeError::Execution(format!("Failed to open system browser: {e}")))?;
            Ok(())
        }
    }

    /// 创建浏览器执行器。
    pub(crate) fn new(config: BrowserConfig) -> Self {
        let _ = std::fs::create_dir_all(&config.screenshot_dir);

        Self {
            config,
            #[cfg(feature = "browser")]
            session: Arc::new(Mutex::new(None)),
        }
    }

    /// 执行浏览器命令。
    pub(crate) async fn execute(&self, cmd: &BrowserCommand) -> NodeResult<CommandOutput> {
        info!("Executing browser command: {:?}", cmd);

        match cmd {
            BrowserCommand::OpenSystem { url } => self.open_system(url).await,
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

    #[cfg_attr(not(feature = "browser"), allow(dead_code))]
    fn output(result: BrowserResult) -> CommandOutput {
        CommandOutput::Browser { result }
    }

    async fn open_system(&self, url: &str) -> NodeResult<CommandOutput> {
        Self::open_url(url)?;

        Ok(Self::output(BrowserResult::OpenSystem {
            url: url.to_string(),
        }))
    }

    #[cfg(feature = "browser")]
    async fn ensure_tab(&self) -> NodeResult<Arc<headless_chrome::Tab>> {
        use headless_chrome::{Browser, LaunchOptionsBuilder};

        let mut session = self.session.lock().await;
        if let Some(session) = session.as_ref() {
            return Ok(session.tab.clone());
        }

        let launch_options = LaunchOptionsBuilder::default()
            .headless(self.config.headless)
            .window_size(Some((self.config.width, self.config.height)))
            .build()
            .map_err(|e| NodeError::Execution(format!("Failed to build browser options: {e}")))?;

        let browser = Browser::new(launch_options)
            .map_err(|e| NodeError::Execution(format!("Failed to launch browser: {e}")))?;
        let tab = browser
            .new_tab()
            .map_err(|e| NodeError::Execution(format!("Failed to create tab: {e}")))?;

        *session = Some(BrowserSession {
            _browser: browser,
            tab: tab.clone(),
        });

        Ok(tab)
    }

    #[cfg(feature = "browser")]
    async fn active_tab(&self) -> NodeResult<Arc<headless_chrome::Tab>> {
        let session = self.session.lock().await;
        session
            .as_ref()
            .map(|session| session.tab.clone())
            .ok_or_else(|| NodeError::Execution("No active browser session".to_string()))
    }

    #[cfg(feature = "browser")]
    fn evaluate_value(
        tab: &headless_chrome::Tab,
        script: &str,
        action: &str,
    ) -> NodeResult<serde_json::Value> {
        tab.evaluate(script, false)
            .map_err(|e| NodeError::Execution(format!("{action} failed: {e}")))
            .map(|result| result.value.unwrap_or(serde_json::Value::Null))
    }

    #[cfg_attr(not(feature = "browser"), allow(dead_code))]
    fn selector_script(selector: &str, expr: &str) -> NodeResult<String> {
        let selector = serde_json::to_string(selector)?;
        Ok(format!(
            "(() => {{ const el = document.querySelector({selector}); if (!el) return null; return {expr}; }})()"
        ))
    }

    /// 导航到 URL。
    #[cfg(feature = "browser")]
    async fn navigate(&self, url: &str) -> NodeResult<CommandOutput> {
        debug!("Navigating to: {}", url);

        let tab = self.ensure_tab().await?;
        tab.navigate_to(url)
            .map_err(|e| NodeError::Execution(format!("Navigation failed: {e}")))?;
        tab.wait_until_navigated()
            .map_err(|e| NodeError::Execution(format!("Wait for navigation failed: {e}")))?;

        let summary = Self::evaluate_value(
            &tab,
            "(() => ({ final_url: window.location.href, title: document.title || null }))()",
            "Read page summary",
        )?;

        let final_url = summary
            .get("final_url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(url)
            .to_string();
        let title = summary
            .get("title")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string);

        Ok(Self::output(BrowserResult::Navigate { final_url, title }))
    }

    /// 导航到 URL（未启用 browser feature）。
    #[cfg(not(feature = "browser"))]
    async fn navigate(&self, _url: &str) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }

    /// 截图。
    #[cfg(feature = "browser")]
    async fn screenshot(
        &self,
        _selector: Option<&str>,
        _full_page: bool,
    ) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Screenshot feature temporarily disabled due to headless_chrome API changes"
                .to_string(),
        ))
    }

    /// 截图（未启用 browser feature）。
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

    /// 点击元素。
    #[cfg(feature = "browser")]
    async fn click(&self, selector: &str) -> NodeResult<CommandOutput> {
        debug!("Clicking element: {}", selector);

        let tab = self.active_tab().await?;
        let element = tab
            .find_element(selector)
            .map_err(|e| NodeError::Execution(format!("Element not found '{selector}': {e}")))?;
        element
            .click()
            .map_err(|e| NodeError::Execution(format!("Click failed: {e}")))?;

        Ok(Self::output(BrowserResult::Ok))
    }

    /// 点击元素（未启用 browser feature）。
    #[cfg(not(feature = "browser"))]
    async fn click(&self, _selector: &str) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }

    /// 输入文本。
    #[cfg(feature = "browser")]
    async fn type_text(&self, selector: &str, text: &str) -> NodeResult<CommandOutput> {
        debug!("Typing into element: {}", selector);

        let tab = self.active_tab().await?;
        let element = tab
            .find_element(selector)
            .map_err(|e| NodeError::Execution(format!("Element not found '{selector}': {e}")))?;
        element
            .click()
            .map_err(|e| NodeError::Execution(format!("Click failed: {e}")))?;
        element
            .type_into(text)
            .map_err(|e| NodeError::Execution(format!("Type failed: {e}")))?;

        Ok(Self::output(BrowserResult::Ok))
    }

    /// 输入文本（未启用 browser feature）。
    #[cfg(not(feature = "browser"))]
    async fn type_text(&self, _selector: &str, _text: &str) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }

    /// 等待元素出现。
    #[cfg(feature = "browser")]
    async fn wait_for(&self, selector: &str, timeout_secs: u64) -> NodeResult<CommandOutput> {
        debug!("Waiting for element: {}", selector);

        let timeout = Duration::from_secs(timeout_secs.max(1));
        let start = Instant::now();

        loop {
            let tab = self.active_tab().await?;
            if tab.find_element(selector).is_ok() {
                return Ok(Self::output(BrowserResult::Ok));
            }

            if start.elapsed() >= timeout {
                return Err(NodeError::Execution(format!(
                    "Timeout waiting for element: {selector}"
                )));
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// 等待元素出现（未启用 browser feature）。
    #[cfg(not(feature = "browser"))]
    async fn wait_for(&self, _selector: &str, _timeout_secs: u64) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }

    /// 获取元素文本。
    #[cfg(feature = "browser")]
    async fn get_text(&self, selector: &str) -> NodeResult<CommandOutput> {
        debug!("Getting text from element: {}", selector);

        let tab = self.active_tab().await?;
        let script = Self::selector_script(selector, "el.textContent ?? ''")?;
        let value = Self::evaluate_value(&tab, &script, "Get text")?;

        let Some(text) = value.as_str() else {
            return Err(NodeError::Execution(format!(
                "Element not found '{selector}'"
            )));
        };

        Ok(Self::output(BrowserResult::GetText {
            text: text.to_string(),
        }))
    }

    /// 获取元素文本（未启用 browser feature）。
    #[cfg(not(feature = "browser"))]
    async fn get_text(&self, _selector: &str) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }

    /// 执行 JavaScript。
    #[cfg(feature = "browser")]
    async fn evaluate(&self, script: &str) -> NodeResult<CommandOutput> {
        debug!(
            "Evaluating JavaScript: {}...",
            &script[..script.len().min(50)]
        );

        let tab = self.active_tab().await?;
        let value = Self::evaluate_value(&tab, script, "Script execution")?;

        Ok(Self::output(BrowserResult::Evaluate { value }))
    }

    /// 执行 JavaScript（未启用 browser feature）。
    #[cfg(not(feature = "browser"))]
    async fn evaluate(&self, _script: &str) -> NodeResult<CommandOutput> {
        Err(NodeError::Execution(
            "Browser support not enabled. Compile with 'browser' feature".to_string(),
        ))
    }

    /// 关闭浏览器。
    #[cfg(feature = "browser")]
    async fn close(&self) -> NodeResult<CommandOutput> {
        debug!("Closing browser");

        let mut session = self.session.lock().await;
        drop(session.take());

        Ok(Self::output(BrowserResult::Ok))
    }

    /// 关闭浏览器（未启用 browser feature）。
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

    #[test]
    fn test_selector_script_escapes_quotes() {
        let script = BrowserExecutor::selector_script("div[data-x='a']", "el.textContent").unwrap();
        assert!(script.contains("document.querySelector(\"div[data-x='a']\")"));
    }

    #[tokio::test]
    async fn test_open_system_returns_browser_output() {
        let executor = BrowserExecutor::default();
        let result = executor.open_system("https://example.com").await.unwrap();

        match result {
            CommandOutput::Browser {
                result: BrowserResult::OpenSystem { url },
            } => assert_eq!(url, "https://example.com"),
            other => panic!("unexpected output: {:?}", other),
        }
    }
}
