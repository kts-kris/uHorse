//! Interactive prompts and selections
//!
//! Provides user interaction utilities using dialoguer.

use dialoguer::{Input, Select, MultiSelect, Confirm, FuzzySelect, Password, Editor, theme::ColorfulTheme};
use std::fmt::Display;

/// Ask user for text input
pub fn prompt(message: &str) -> Result<String, dialoguer::Error> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .interact_text()
}

/// Ask user for text input with default value
pub fn prompt_with_default(message: &str, default: &str) -> Result<String, dialoguer::Error> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .default(default.to_string())
        .interact_text()
}

/// Ask user for text input with validation
pub fn prompt_validated<F>(message: &str, validator: F) -> Result<String, dialoguer::Error>
where
    F: Fn(&str) -> Result<(), String> + Sync + Send,
{
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .validate_with(validator)
        .interact_text()
}

/// Ask user for password (hidden input)
pub fn prompt_password(message: &str) -> Result<String, dialoguer::Error> {
    Password::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .interact()
}

/// Ask user for password with confirmation
pub fn prompt_password_confirm(message: &str) -> Result<String, dialoguer::Error> {
    Password::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .with_confirmation("Confirm password", "Passwords do not match")
        .interact()
}

/// Ask user to select from a list
pub fn select<T: Display>(message: &str, items: &[T]) -> Result<usize, dialoguer::Error> {
    Select::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .items(items)
        .default(0)
        .interact()
}

/// Ask user to select from a list with fuzzy search
pub fn fuzzy_select<T: Display>(message: &str, items: &[T]) -> Result<usize, dialoguer::Error> {
    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .items(items)
        .default(0)
        .interact()
}

/// Ask user to select multiple items
pub fn multi_select<T: Display>(message: &str, items: &[T]) -> Result<Vec<usize>, dialoguer::Error> {
    MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .items(items)
        .interact()
}

/// Ask user for yes/no confirmation
pub fn confirm(message: &str) -> Result<bool, dialoguer::Error> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .default(false)
        .interact()
}

/// Ask user for yes/no confirmation with default true
pub fn confirm_default_yes(message: &str) -> Result<bool, dialoguer::Error> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(message)
        .default(true)
        .interact()
}

/// Open text editor for multi-line input
pub fn edit(message: &str, initial: &str) -> Result<String, dialoguer::Error> {
    Editor::new()
        .edit(initial)
        .map(|s| s.unwrap_or_default())
}

/// Interactive menu for selecting actions
pub struct ActionMenu {
    title: String,
    actions: Vec<String>,
}

impl ActionMenu {
    /// Create a new action menu
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            actions: Vec::new(),
        }
    }

    /// Add an action to the menu
    pub fn action(mut self, name: &str) -> Self {
        self.actions.push(name.to_string());
        self
    }

    /// Display the menu and get selection
    pub fn show(&self) -> Result<usize, dialoguer::Error> {
        select(&self.title, &self.actions)
    }
}

/// Channel selection helper
pub fn select_channels() -> Result<Vec<String>, dialoguer::Error> {
    let channels = [
        "Telegram",
        "DingTalk (钉钉)",
        "Feishu (飞书)",
        "WeCom (企业微信)",
        "Slack",
        "Discord",
        "WhatsApp",
    ];

    let selected = multi_select("Select channels to enable (Space to select, Enter to confirm)", &channels)?;

    let channel_ids = ["telegram", "dingtalk", "feishu", "wecom", "slack", "discord", "whatsapp"];
    Ok(selected.into_iter().map(|i| channel_ids[i].to_string()).collect())
}

/// LLM provider selection helper
pub fn select_llm_provider() -> Result<String, dialoguer::Error> {
    let providers = [
        "OpenAI (GPT-4, GPT-3.5)",
        "Anthropic (Claude)",
        "Google (Gemini)",
        "Azure OpenAI",
        "AWS Bedrock",
        "Local (Ollama)",
        "Custom (OpenAI-compatible)",
    ];

    let provider_ids = ["openai", "anthropic", "google", "azure", "aws", "local", "custom"];
    let selection = select("Select LLM provider", &providers)?;
    Ok(provider_ids[selection].to_string())
}

/// Database type selection helper
pub fn select_database() -> Result<String, dialoguer::Error> {
    let databases = [
        "SQLite (Recommended for single server)",
        "PostgreSQL (Recommended for production cluster)",
    ];

    let db_types = ["sqlite", "postgres"];
    let selection = select("Select database type", &databases)?;
    Ok(db_types[selection].to_string())
}

/// Deployment mode selection helper
pub fn select_deployment_mode() -> Result<String, dialoguer::Error> {
    let modes = [
        "Single server (Development/Testing)",
        "High availability cluster (Production)",
    ];

    let mode_ids = ["single", "cluster"];
    let selection = select("Select deployment mode", &modes)?;
    Ok(mode_ids[selection].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_menu() {
        let menu = ActionMenu::new("Main Menu")
            .action("Start server")
            .action("Configure")
            .action("View logs")
            .action("Exit");

        assert_eq!(menu.actions.len(), 4);
    }
}
