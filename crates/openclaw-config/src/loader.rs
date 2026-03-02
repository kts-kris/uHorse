//! # 配置加载器
//!
//! 支持从文件、环境变量等多种来源加载配置。

use super::{OpenClawConfig, ConfigSource, ConfigValue, MergeStrategy};
use anyhow::{Context, Result as AnyhowResult};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// 配置加载器
pub struct ConfigLoader {
    sources: Vec<Box<dyn ConfigSource>>,
    merge_strategy: MergeStrategy,
}

impl ConfigLoader {
    /// 创建新的加载器
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            merge_strategy: MergeStrategy::Default,
        }
    }

    /// 添加配置源
    pub fn add_source(mut self, source: Box<dyn ConfigSource>) -> Self {
        self.sources.push(source);
        self
    }

    /// 设置合并策略
    pub fn with_merge_strategy(mut self, strategy: MergeStrategy) -> Self {
        self.merge_strategy = strategy;
        self
    }

    /// 加载配置
    pub fn load(&self) -> AnyhowResult<OpenClawConfig> {
        let mut config = OpenClawConfig::default();

        for source in &self.sources {
            match source.load() {
                Ok(value) => {
                    debug!("Loaded config from: {}", source.name());
                    config = self.merge(config, value)?;
                }
                Err(e) => {
                    warn!("Failed to load config from {}: {}", source.name(), e);
                    // 某些源可能是可选的，继续加载其他源
                }
            }
        }

        // 应用环境变量覆盖
        self.apply_env_overrides(&mut config)?;

        Ok(config)
    }

    /// 合并配置
    fn merge(&self, base: OpenClawConfig, overlay: ConfigValue) -> AnyhowResult<OpenClawConfig> {
        match self.merge_strategy {
            MergeStrategy::Default => {
                // 使用 overlay 替换 base 中的值
                let overlay_json = overlay.as_json()?;
                let base_json = serde_json::to_value(&base)?;
                let merged = Self::deep_merge(base_json, overlay_json);
                serde_json::from_value(merged).context("Failed to deserialize merged config")
            }
            MergeStrategy::Override => {
                // 完全使用 overlay
                serde_json::from_value(overlay.as_json()?)
                    .context("Failed to deserialize override config")
            }
        }
    }

    /// 深度合并两个 JSON 值
    fn deep_merge(base: serde_json::Value, overlay: serde_json::Value) -> serde_json::Value {
        match (base, overlay) {
            (serde_json::Value::Object(mut base_map), serde_json::Value::Object(overlay_map)) => {
                for (key, overlay_value) in overlay_map {
                    let merged = if let Some(base_value) = base_map.remove(&key) {
                        Self::deep_merge(base_value, overlay_value)
                    } else {
                        overlay_value
                    };
                    base_map.insert(key, merged);
                }
                serde_json::Value::Object(base_map)
            }
            (_, overlay) => overlay,
        }
    }

    /// 应用环境变量覆盖
    fn apply_env_overrides(&self, config: &mut OpenClawConfig) -> AnyhowResult<()> {
        // OPENCLAW_SERVER_HOST -> server.host
        if let Ok(host) = std::env::var("OPENCLAW_SERVER_HOST") {
            config.server.host = host;
        }
        if let Ok(port) = std::env::var("OPENCLAW_SERVER_PORT") {
            config.server.port = port.parse().context("Invalid OPENCLAW_SERVER_PORT")?;
        }

        // OPENCLAW_DATABASE_PATH -> database.path
        if let Ok(path) = std::env::var("OPENCLAW_DATABASE_PATH") {
            config.database.path = path;
        }

        // OPENCLAW_JWT_SECRET -> security.jwt_secret
        if let Ok(secret) = std::env::var("OPENCLAW_JWT_SECRET") {
            config.security.jwt_secret = Some(secret);
        }

        // OPENCLAW_LOG_LEVEL -> logging.level
        if let Ok(level) = std::env::var("OPENCLAW_LOG_LEVEL") {
            config.logging.level = level;
        }

        // OPENCLAW_TELEGRAM_BOT_TOKEN -> channels.telegram.bot_token
        if let Some(telegram) = &mut config.channels.telegram {
            if let Ok(token) = std::env::var("OPENCLAW_TELEGRAM_BOT_TOKEN") {
                telegram.bot_token = token;
            }
        }

        Ok(())
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// 配置监视器（支持热加载）
pub struct ConfigWatch {
    config: Arc<RwLock<OpenClawConfig>>,
    loader: ConfigLoader,
    config_path: PathBuf,
}

impl ConfigWatch {
    /// 创建新的配置监视器
    pub fn new(loader: ConfigLoader, config_path: PathBuf) -> Self {
        Self {
            config: Arc::new(RwLock::new(OpenClawConfig::default())),
            loader,
            config_path,
        }
    }

    /// 初始化配置
    pub async fn init(&self) -> AnyhowResult<()> {
        let config = self.loader.load()?;
        *self.config.write().await = config;
        info!("Configuration initialized");
        Ok(())
    }

    /// 获取配置快照
    pub async fn get(&self) -> OpenClawConfig {
        self.config.read().await.clone()
    }

    /// 重新加载配置
    pub async fn reload(&self) -> AnyhowResult<()> {
        let config = self.loader.load()?;
        *self.config.write().await = config;
        info!("Configuration reloaded");
        Ok(())
    }

    /// 启动配置监视任务
    pub async fn spawn_watch_task(&self) -> AnyhowResult<()> {
        use tokio::time::{interval, Duration};

        let _config = Arc::clone(&self.config);
        let loader_config_path = self.config_path.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(5));
            let mut last_modified = Self::get_modified_time(&loader_config_path).ok();

            loop {
                ticker.tick().await;

                if let Ok(current_modified) = Self::get_modified_time(&loader_config_path) {
                    if let Some(last) = last_modified {
                        if current_modified > last {
                            info!("Configuration file changed, triggering reload");
                            // TODO: 触发配置重新加载
                            last_modified = Some(current_modified);
                        }
                    } else {
                        last_modified = Some(current_modified);
                    }
                }
            }
        });

        Ok(())
    }

    /// 获取文件修改时间
    fn get_modified_time(path: &Path) -> AnyhowResult<std::time::SystemTime> {
        let metadata = std::fs::metadata(path)?;
        Ok(metadata.modified()?)
    }
}

/// 创建默认配置加载器
pub fn create_default_loader(config_path: &Path) -> ConfigLoader {
    use super::source::{FileSource, EnvSource};

    ConfigLoader::new()
        .add_source(Box::new(FileSource::new(config_path.to_path_buf())))
        .add_source(Box::new(EnvSource::new()))
}
