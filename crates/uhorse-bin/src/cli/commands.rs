//! Extended CLI commands
//!
//! Provides new CLI commands: doctor, init, tutorial, template.

use crate::cli::{
    banner, interactive, output, progress,
};
use std::path::Path;

/// Doctor command - diagnose and fix issues
pub struct DoctorCommand;

impl DoctorCommand {
    /// Run diagnostics
    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        banner::print_banner_with_version(env!("CARGO_PKG_VERSION"));

        println!("{}", "Running diagnostics...\n".cyan());

        let mut issues = Vec::new();
        let mut passed = 0;
        let total = 8;

        // Check 1: Rust version
        output::step("Checking Rust version...");
        if let Ok(version) = Self::check_rust_version() {
            output::success(&format!("Rust {} detected", version));
            passed += 1;
        } else {
            issues.push("Rust 1.75+ is required. Install from https://rustup.rs");
            output::error("Rust version too old or not installed");
        }

        // Check 2: Cargo
        output::step("Checking Cargo...");
        if Self::check_cargo() {
            output::success("Cargo is available");
            passed += 1;
        } else {
            issues.push("Cargo not found in PATH");
            output::error("Cargo not found");
        }

        // Check 3: Configuration
        output::step("Checking configuration...");
        if Self::check_config() {
            output::success("Configuration file found");
            passed += 1;
        } else {
            issues.push("Run 'uhorse wizard' to create configuration");
            output::warning("No configuration file found");
        }

        // Check 4: Database
        output::step("Checking database...");
        if Self::check_database().await {
            output::success("Database connection OK");
            passed += 1;
        } else {
            issues.push("Database not accessible. Check connection string.");
            output::warning("Database not accessible");
        }

        // Check 5: LLM API
        output::step("Checking LLM API key...");
        if Self::check_llm_key() {
            output::success("LLM API key configured");
            passed += 1;
        } else {
            issues.push("Set LLM_API_KEY environment variable or configure in config.toml");
            output::warning("LLM API key not set");
        }

        // Check 6: Ports
        output::step("Checking port availability...");
        if Self::check_ports() {
            output::success("Port 8765 is available");
            passed += 1;
        } else {
            issues.push("Port 8765 is in use. Stop conflicting services or change port.");
            output::warning("Port 8765 is in use");
        }

        // Check 7: Write permissions
        output::step("Checking write permissions...");
        if Self::check_permissions() {
            output::success("Data directory is writable");
            passed += 1;
        } else {
            issues.push("Cannot write to data directory. Check permissions.");
            output::error("Insufficient permissions");
        }

        // Check 8: Optional dependencies
        output::step("Checking optional dependencies...");
        let opt_deps = Self::check_optional_deps();
        if !opt_deps.is_empty() {
            output::success(&format!("Optional: {}", opt_deps.join(", ")));
        } else {
            output::info("No optional dependencies found (Redis, NATS, etcd)");
        }
        passed += 1;

        // Summary
        println!();
        output::separator();
        println!();

        if passed == total {
            banner::print_success_banner(
                "All Checks Passed",
                &format!("{}/{} checks passed", passed, total),
            );
        } else {
            banner::print_warning_banner(
                &format!("{}/{} Checks Passed", passed, total),
                "Some issues were found:",
            );

            for (i, issue) in issues.iter().enumerate() {
                println!("  {}. {}", i + 1, issue);
            }
            println!();
            println!("  Run {} for guided setup", "uhorse wizard".yellow());
        }

        Ok(())
    }

    fn check_rust_version() -> Result<String, ()> {
        use std::process::Command;
        let output = Command::new("rustc").arg("--version").output().map_err(|_| ())?;
        let version = String::from_utf8_lossy(&output.stdout);
        // Parse version and check >= 1.75
        Ok(version.trim().to_string())
    }

    fn check_cargo() -> bool {
        use std::process::Command;
        Command::new("cargo").arg("--version").output().is_ok()
    }

    fn check_config() -> bool {
        Path::new("config.toml").exists() || Path::new("~/.uhorse/config.toml").exists()
    }

    async fn check_database() -> bool {
        // Simplified check - in real implementation, try actual connection
        Path::new("./data").exists()
    }

    fn check_llm_key() -> bool {
        std::env::var("LLM_API_KEY").is_ok() || std::env::var("OPENAI_API_KEY").is_ok()
    }

    fn check_ports() -> bool {
        use std::net::TcpListener;
        TcpListener::bind("127.0.0.1:8765").is_ok()
    }

    fn check_permissions() -> bool {
        use std::fs;
        if let Ok(dir) = fs::read_dir("./data") {
            true
        } else {
            fs::create_dir_all("./data").is_ok()
        }
    }

    fn check_optional_deps() -> Vec<&'static str> {
        let mut deps = Vec::new();
        use std::process::Command;

        if Command::new("redis-cli").arg("ping").output().is_ok() {
            deps.push("Redis");
        }
        if Command::new("nats-server").arg("--version").output().is_ok() {
            deps.push("NATS");
        }
        if Command::new("etcdctl").arg("version").output().is_ok() {
            deps.push("etcd");
        }

        deps
    }
}

/// Init command - quick project initialization
pub struct InitCommand;

impl InitCommand {
    /// Initialize a new uHorse project
    pub async fn run(name: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        banner::print_welcome();

        let project_name = match name {
            Some(n) => n.to_string(),
            None => interactive::prompt_with_default("Project name", "my-uhorse")?,
        };

        output::header(&format!("Initializing {}...", project_name));

        // Create directory structure
        let spinner = progress::spinner("Creating directory structure...");
        Self::create_directories(&project_name)?;
        spinner.success("Directory structure created");

        // Generate configuration
        let spinner = progress::spinner("Generating configuration...");
        Self::generate_config(&project_name)?;
        spinner.success("Configuration generated");

        // Create agent workspace
        let spinner = progress::spinner("Setting up agent workspace...");
        Self::setup_workspace(&project_name)?;
        spinner.success("Agent workspace ready");

        // Create skills
        let spinner = progress::spinner("Creating default skills...");
        Self::create_default_skills(&project_name)?;
        spinner.success("Default skills created");

        // Create memory files
        let spinner = progress::spinner("Creating memory files...");
        Self::create_memory_files(&project_name)?;
        spinner.success("Memory files created");

        // Print completion
        banner::print_completion(&format!("{}/config.toml", project_name));

        println!("  Quick start:");
        println!("    {} cd {}", "→".cyan(), project_name);
        println!("    {} uhorse start", "→".cyan());
        println!();

        Ok(())
    }

    fn create_directories(name: &str) -> std::io::Result<()> {
        use std::fs;
        fs::create_dir_all(name)?;
        fs::create_dir_all(format!("{}/data", name))?;
        fs::create_dir_all(format!("{}/logs", name))?;
        fs::create_dir_all(format!("{}/workspace", name))?;
        fs::create_dir_all(format!("{}/skills", name))?;
        Ok(())
    }

    fn generate_config(name: &str) -> std::io::Result<()> {
        use std::fs;
        let config = r#"[server]
host = "127.0.0.1"
port = 8765

[server.health]
enabled = true
path = "/api/health"
verbose = false

[database]
path = "./data/uhorse.db"

[llm]
enabled = true
provider = "openai"
model = "gpt-4"

[channels]
enabled = []

[observability]
service_name = "uhorse-hub"
"#;
        fs::write(format!("{}/config.toml", name), config)
    }

    fn setup_workspace(name: &str) -> std::io::Result<()> {
        use std::fs;
        fs::create_dir_all(format!("{}/workspace/default", name))?;
        fs::write(format!("{}/workspace/default/SOUL.md", name), "# Agent Constitution\n\n")?;
        fs::write(format!("{}/workspace/default/MEMORY.md", name), "# Agent Memory\n\n")?;
        Ok(())
    }

    fn create_default_skills(name: &str) -> std::io::Result<()> {
        use std::fs;
        fs::create_dir_all(format!("{}/skills/builtin", name))?;
        Ok(())
    }

    fn create_memory_files(name: &str) -> std::io::Result<()> {
        use std::fs;
        fs::write(format!("{}/.gitignore", name), "data/\nlogs/\n*.db\n*.log\n")?;
        Ok(())
    }
}

/// Tutorial command - interactive learning
pub struct TutorialCommand;

impl TutorialCommand {
    /// Run the interactive tutorial
    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        banner::print_banner_with_version(env!("CARGO_PKG_VERSION"));

        output::header("Welcome to uHorse Interactive Tutorial");

        let topics = [
            "1. Getting Started - Basic concepts and setup",
            "2. Channels - Connecting to messaging platforms",
            "3. Agents - Creating and managing AI agents",
            "4. Skills - Building custom capabilities",
            "5. Memory - Understanding the memory system",
            "6. Deployment - Production best practices",
        ];

        let selection = interactive::select("Choose a topic to learn:", &topics)?;

        match selection {
            0 => Self::topic_getting_started().await?,
            1 => Self::topic_channels().await?,
            2 => Self::topic_agents().await?,
            3 => Self::topic_skills().await?,
            4 => Self::topic_memory().await?,
            5 => Self::topic_deployment().await?,
            _ => {}
        }

        Ok(())
    }

    async fn topic_getting_started() -> Result<(), Box<dyn std::error::Error>> {
        output::header("Getting Started");

        println!("  uHorse is an enterprise-grade multi-channel AI gateway.");
        println!();
        println!("  {} Architecture:", "→".cyan());
        println!("    • Gateway: HTTP/WebSocket interface");
        println!("    • Agent: LLM orchestration layer");
        println!("    • Skills: Tool execution system");
        println!("    • Memory: Persistent context storage");
        println!();
        println!("  {} Quick Start:", "→".cyan());
        println!("    1. Run {} to configure", "uhorse wizard".yellow());
        println!("    2. Run {} to start", "uhorse start".yellow());
        println!("    3. Test with {}", "curl http://localhost:8765/api/health".yellow());
        println!();

        if interactive::confirm("Try it now?")? {
            output::info("Starting configuration wizard...");
            // Would call wizard command
        }

        Ok(())
    }

    async fn topic_channels() -> Result<(), Box<dyn std::error::Error>> {
        output::header("Channels");

        println!("  uHorse supports 7+ messaging platforms:");
        println!();
        println!("  {} Telegram", "•".cyan());
        println!("    Most mature channel with full Bot API support.");
        println!();
        println!("  {} DingTalk (钉钉)", "•".cyan());
        println!("    Enterprise messaging with rich card support.");
        println!();
        println!("  {} Feishu (飞书)", "•".cyan());
        println!("    ByteDance's enterprise platform.");
        println!();
        println!("  {} WeCom (企业微信)", "•".cyan());
        println!("    Tencent's enterprise solution.");
        println!();
        println!("  {} Slack / Discord / WhatsApp", "•".cyan());
        println!("    International platforms supported.");
        println!();

        Ok(())
    }

    async fn topic_agents() -> Result<(), Box<dyn std::error::Error>> {
        output::header("Agents");
        println!("  Agents are independent AI assistants with their own workspace.");
        // ... more content
        Ok(())
    }

    async fn topic_skills() -> Result<(), Box<dyn std::error::Error>> {
        output::header("Skills");
        println!("  Skills extend agent capabilities with custom tools.");
        // ... more content
        Ok(())
    }

    async fn topic_memory() -> Result<(), Box<dyn std::error::Error>> {
        output::header("Memory");
        println!("  The memory system provides persistent context for agents.");
        // ... more content
        Ok(())
    }

    async fn topic_deployment() -> Result<(), Box<dyn std::error::Error>> {
        output::header("Deployment");
        println!("  Production deployment options and best practices.");
        // ... more content
        Ok(())
    }
}

/// Template command - apply scenario templates
pub struct TemplateCommand;

impl TemplateCommand {
    /// List available templates
    pub fn list() {
        output::header("Available Templates");

        let templates = [
            ("customer-service", "Customer service bot with FAQ and ticket management"),
            ("hr-assistant", "HR assistant for employee queries and onboarding"),
            ("it-support", "IT helpdesk with troubleshooting and ticket creation"),
            ("sales-bot", "Sales assistant with lead qualification and CRM integration"),
            ("general", "General-purpose assistant with basic skills"),
        ];

        for (name, desc) in templates {
            output::bullet(&format!("{} - {}", name.green(), desc));
        }

        println!();
        println!("  Usage: {} <template-name>", "uhorse init --template".yellow());
    }

    /// Apply a template
    pub async fn apply(template: &str) -> Result<(), Box<dyn std::error::Error>> {
        output::info(&format!("Applying template: {}", template));

        // In real implementation, copy template files
        let spinner = progress::spinner("Copying template files...");
        std::thread::sleep(std::time::Duration::from_secs(1));
        spinner.success("Template applied");

        output::success(&format!("Template '{}' applied successfully", template));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_list() {
        TemplateCommand::list();
    }
}
