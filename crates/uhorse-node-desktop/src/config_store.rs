use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sysinfo::System;
use uhorse_node_runtime::{NodeConfig, NodeError, NodeId, NodeResult};
use uhorse_protocol::{CommandType, NodeCapabilities};

const FALLBACK_NODE_NAME: &str = "uHorse-Node";
const DEFAULT_NODE_ID_PREFIX: &str = "node-desktop-";

fn desktop_node_capabilities() -> NodeCapabilities {
    NodeCapabilities {
        supported_commands: vec![
            CommandType::File,
            CommandType::Shell,
            CommandType::Code,
            CommandType::Browser,
        ],
        ..NodeCapabilities::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopConfig {
    #[serde(flatten)]
    pub node: NodeConfig,
    #[serde(default)]
    pub desktop: DesktopPreferencesConfig,
}

impl Default for DesktopConfig {
    fn default() -> Self {
        let mut node = NodeConfig::default();
        node.name = current_computer_name();
        node.node_id = Some(default_node_id(&node.name));
        node.capabilities = desktop_node_capabilities();
        Self {
            node,
            desktop: DesktopPreferencesConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopPreferencesConfig {
    #[serde(default = "default_notifications_enabled")]
    pub notifications_enabled: bool,
    #[serde(default = "default_show_notification_details")]
    pub show_notification_details: bool,
    #[serde(default = "default_mirror_notifications_to_dingtalk")]
    pub mirror_notifications_to_dingtalk: bool,
    #[serde(default)]
    pub launch_at_login: bool,
}

impl Default for DesktopPreferencesConfig {
    fn default() -> Self {
        Self {
            notifications_enabled: default_notifications_enabled(),
            show_notification_details: default_show_notification_details(),
            mirror_notifications_to_dingtalk: default_mirror_notifications_to_dingtalk(),
            launch_at_login: false,
        }
    }
}

fn default_notifications_enabled() -> bool {
    true
}

fn default_show_notification_details() -> bool {
    true
}

fn default_mirror_notifications_to_dingtalk() -> bool {
    false
}

fn default_node_id(name: &str) -> NodeId {
    let normalized = name
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' => ch,
            'A'..='Z' => ch.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>();
    let collapsed = normalized
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let suffix = if collapsed.is_empty() {
        "default".to_string()
    } else {
        collapsed
    };
    NodeId::from_string(format!("{}{}", DEFAULT_NODE_ID_PREFIX, suffix))
}

pub fn current_computer_name() -> String {
    System::host_name()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| FALLBACK_NODE_NAME.to_string())
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> NodeResult<DesktopConfig> {
        if !self.path.exists() {
            return Ok(DesktopConfig::default());
        }

        let content = fs::read_to_string(&self.path).map_err(NodeError::Io)?;
        let mut config: DesktopConfig = toml::from_str(&content)
            .map_err(|error| NodeError::Config(format!("Failed to parse config: {}", error)))?;

        if config.node.node_id.is_none() {
            config.node.node_id = Some(default_node_id(&config.node.name));
        }
        if !config
            .node
            .capabilities
            .supported_commands
            .contains(&CommandType::Browser)
        {
            config
                .node
                .capabilities
                .supported_commands
                .push(CommandType::Browser);
        }

        Ok(config)
    }

    pub fn save(&self, config: &DesktopConfig) -> NodeResult<()> {
        let content = toml::to_string_pretty(config)
            .map_err(|error| NodeError::Config(format!("Failed to serialize config: {}", error)))?;
        fs::write(&self.path, content).map_err(NodeError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use uhorse_protocol::CommandType;

    #[test]
    fn test_load_returns_default_when_missing() {
        let temp = TempDir::new().unwrap();
        let store = ConfigStore::new(temp.path().join("missing.toml"));

        let config = store.load().unwrap();
        assert!(!config.node.name.trim().is_empty());
        assert!(config.node.node_id.is_some());
        assert!(config.desktop.notifications_enabled);
        assert!(config.desktop.show_notification_details);
        assert!(!config.desktop.mirror_notifications_to_dingtalk);
        assert!(!config.desktop.launch_at_login);
    }

    #[test]
    fn test_save_and_load_round_trip() {
        let temp = TempDir::new().unwrap();
        let store = ConfigStore::new(temp.path().join("node-desktop.toml"));
        let mut config = DesktopConfig::default();
        config.node.name = "Desktop Node".to_string();
        config.node.node_id = Some(NodeId::from_string("node-desktop-test"));
        config.node.workspace_path = temp.path().to_string_lossy().to_string();
        config.desktop.notifications_enabled = false;
        config.desktop.show_notification_details = false;
        config.desktop.mirror_notifications_to_dingtalk = true;
        config.desktop.launch_at_login = true;

        store.save(&config).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded.node.name, "Desktop Node");
        assert_eq!(
            loaded.node.node_id.as_ref().map(NodeId::as_str),
            Some("node-desktop-test")
        );
        assert_eq!(loaded.node.workspace_path, config.node.workspace_path);
        assert!(!loaded.desktop.notifications_enabled);
        assert!(!loaded.desktop.show_notification_details);
        assert!(loaded.desktop.mirror_notifications_to_dingtalk);
        assert!(loaded.desktop.launch_at_login);
    }

    #[test]
    fn test_load_backfills_stable_node_id_for_legacy_config() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("node-desktop.toml");
        fs::write(
            &config_path,
            r#"name = "My Desktop"
workspace_path = "/tmp/workspace"
require_git_repo = false

[desktop]
notifications_enabled = true
show_notification_details = true
mirror_notifications_to_dingtalk = false
launch_at_login = false
"#,
        )
        .unwrap();

        let loaded = ConfigStore::new(config_path).load().unwrap();
        assert_eq!(
            loaded.node.node_id.as_ref().map(NodeId::as_str),
            Some("node-desktop-my-desktop")
        );
    }

    #[test]
    fn test_default_config_enables_browser_capability() {
        let config = DesktopConfig::default();
        assert!(config
            .node
            .capabilities
            .supported_commands
            .contains(&CommandType::Browser));
    }

    #[test]
    fn test_load_backfills_browser_capability_for_legacy_config() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("node-desktop.toml");
        fs::write(
            &config_path,
            r#"name = "Legacy Desktop"
workspace_path = "/tmp/workspace"
require_git_repo = false

[capabilities]
supported_commands = ["file", "shell", "code"]
tags = ["default"]
max_concurrent_tasks = 5
available_tools = []

[desktop]
notifications_enabled = true
show_notification_details = true
mirror_notifications_to_dingtalk = false
launch_at_login = false
"#,
        )
        .unwrap();

        let loaded = ConfigStore::new(config_path).load().unwrap();
        assert!(loaded
            .node
            .capabilities
            .supported_commands
            .contains(&CommandType::Browser));
    }
}
