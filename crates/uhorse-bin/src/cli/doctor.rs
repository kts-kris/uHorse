//! Enhanced Doctor Command
//!
//! Provides diagnostics with auto-fix capabilities.

use crate::cli::{banner, output, progress};
use std::path::Path;
use std::process::Command;

/// Diagnostic check result
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
    pub fix: Option<FixAction>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CheckStatus {
    Ok,
    Warning,
    Error,
}

/// Auto-fix action
#[derive(Debug, Clone)]
pub struct FixAction {
    pub description: String,
    pub action: FixType,
}

#[derive(Debug, Clone)]
pub enum FixType {
    /// Create a file
    CreateFile { path: String, content: String },
    /// Create a directory
    CreateDir { path: String },
    /// Set environment variable instruction
    SetEnvVar { name: String, description: String },
    /// Run a command
    RunCommand { command: String, args: Vec<String> },
    /// Manual action required
    Manual { instructions: String },
}

/// Enhanced doctor command
pub struct EnhancedDoctor;

impl EnhancedDoctor {
    /// Run all diagnostics
    pub async fn run(fix: bool) -> Result<(), Box<dyn std::error::Error>> {
        banner::print_banner_with_version(env!("CARGO_PKG_VERSION"));

        println!("{}", "Running diagnostics...\n".cyan());

        let checks = vec![
            ("Rust Version", Self::check_rust()),
            ("Cargo", Self::check_cargo()),
            ("Configuration", Self::check_config()),
            ("Database", Self::check_database().await),
            ("LLM API Key", Self::check_llm_key()),
            ("Data Directory", Self::check_data_dir()),
            ("Port 8080", Self::check_port()),
            ("Dependencies", Self::check_deps()),
        ];

        let mut results = Vec::new();
        let mut passed = 0;
        let mut warnings = 0;
        let mut errors = 0;

        for (name, result) in checks {
            output::step(&format!("Checking {}...", name));

            match result.status {
                CheckStatus::Ok => {
                    output::success(&result.message);
                    passed += 1;
                }
                CheckStatus::Warning => {
                    output::warning(&result.message);
                    warnings += 1;
                }
                CheckStatus::Error => {
                    output::error(&result.message);
                    errors += 1;
                }
            }

            results.push(result);
        }

        // Summary
        println!();
        output::separator();
        println!();

        let total = results.len();
        if errors == 0 && warnings == 0 {
            banner::print_success_banner(
                "All Checks Passed",
                &format!("{}/{} checks passed", passed, total),
            );
        } else {
            println!(
                "  Results: {} {} | {} {} | {} {}",
                passed.to_string().green(),
                "passed",
                warnings.to_string().yellow(),
                "warnings",
                errors.to_string().red(),
                "errors",
            );
            println!();

            // Show issues
            if errors > 0 || warnings > 0 {
                println!("{}", "Issues found:".bold());
                println!();

                for result in &results {
                    if result.status != CheckStatus::Ok {
                        let icon = match result.status {
                            CheckStatus::Error => "✗".red(),
                            CheckStatus::Warning => "⚠".yellow(),
                            CheckStatus::Ok => "✓".green(),
                        };
                        println!("  {} {}", icon, result.message);

                        if let Some(ref fix_action) = result.fix {
                            println!("    {} {}", "→".cyan(), fix_action.description);
                        }
                    }
                }
                println!();
            }

            // Auto-fix if requested
            if fix && (errors > 0 || warnings > 0) {
                Self::apply_fixes(&results)?;
            } else if errors > 0 || warnings > 0 {
                println!("  Run {} to automatically fix issues", "uhorse doctor --fix".yellow());
            }
        }

        Ok(())
    }

    /// Apply auto-fixes
    fn apply_fixes(results: &[CheckResult]) -> Result<(), Box<dyn std::error::Error>> {
        println!("{}", "Applying fixes...".cyan());
        println!();

        for result in results {
            if let Some(ref fix) = result.fix {
                let spinner = progress::spinner(&format!("Fixing: {}...", result.name));

                match &fix.action {
                    FixType::CreateFile { path, content } => {
                        if let Some(parent) = Path::new(path).parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        std::fs::write(path, content)?;
                        spinner.success(&format!("Created {}", path));
                    }
                    FixType::CreateDir { path } => {
                        std::fs::create_dir_all(path)?;
                        spinner.success(&format!("Created directory {}", path));
                    }
                    FixType::SetEnvVar { name, description } => {
                        spinner.warning(&format!("Manual action required: {} - {}", name, description));
                    }
                    FixType::RunCommand { command, args } => {
                        let status = Command::new(command)
                            .args(args)
                            .status()?;
                        if status.success() {
                            spinner.success("Command executed successfully");
                        } else {
                            spinner.error("Command failed");
                        }
                    }
                    FixType::Manual { instructions } => {
                        spinner.warning(&format!("Manual action: {}", instructions));
                    }
                }
            }
        }

        println!();
        output::success("Fixes applied. Run 'uhorse doctor' again to verify.");

        Ok(())
    }

    // Individual checks

    fn check_rust() -> CheckResult {
        match Command::new("rustc").arg("--version").output() {
            Ok(output) => {
                let version = String::from_utf8_lossy(&output.stdout);
                let version_str = version.trim();

                // Parse version
                if let Some(version_num) = version_str.split_whitespace().nth(1) {
                    let parts: Vec<u32> = version_num
                        .split('.')
                        .filter_map(|s| s.parse().ok())
                        .collect();

                    if parts.len() >= 2 {
                        let major = parts[0];
                        let minor = parts[1];

                        if major > 1 || (major == 1 && minor >= 75) {
                            return CheckResult {
                                name: "Rust".to_string(),
                                status: CheckStatus::Ok,
                                message: format!("Rust {} detected", version_num),
                                fix: None,
                            };
                        }
                    }
                }

                CheckResult {
                    name: "Rust".to_string(),
                    status: CheckStatus::Warning,
                    message: "Rust version might be too old (1.75+ recommended)".to_string(),
                    fix: Some(FixAction {
                        description: "Run 'rustup update' to upgrade Rust".to_string(),
                        action: FixType::Manual {
                            instructions: "Run: rustup update".to_string(),
                        },
                    }),
                }
            }
            Err(_) => CheckResult {
                name: "Rust".to_string(),
                status: CheckStatus::Error,
                message: "Rust not installed".to_string(),
                fix: Some(FixAction {
                    description: "Install Rust from https://rustup.rs".to_string(),
                    action: FixType::Manual {
                        instructions: "Visit https://rustup.rs and follow installation instructions".to_string(),
                    },
                }),
            },
        }
    }

    fn check_cargo() -> CheckResult {
        match Command::new("cargo").arg("--version").output() {
            Ok(output) => {
                let version = String::from_utf8_lossy(&output.stdout);
                CheckResult {
                    name: "Cargo".to_string(),
                    status: CheckStatus::Ok,
                    message: format!("Cargo {}", version.trim()),
                    fix: None,
                }
            }
            Err(_) => CheckResult {
                name: "Cargo".to_string(),
                status: CheckStatus::Error,
                message: "Cargo not found in PATH".to_string(),
                fix: Some(FixAction {
                    description: "Install Rust which includes Cargo".to_string(),
                    action: FixType::Manual {
                        instructions: "Visit https://rustup.rs".to_string(),
                    },
                }),
            },
        }
    }

    fn check_config() -> CheckResult {
        let config_paths = ["config.toml", ".uhorse/config.toml", "~/.uhorse/config.toml"];

        for path in &config_paths {
            let expanded = if path.starts_with("~") {
                shellexpand::tilde(path).to_string()
            } else {
                path.to_string()
            };

            if Path::new(&expanded).exists() {
                return CheckResult {
                    name: "Configuration".to_string(),
                    status: CheckStatus::Ok,
                    message: format!("Configuration found at {}", path),
                    fix: None,
                };
            }
        }

        CheckResult {
            name: "Configuration".to_string(),
            status: CheckStatus::Warning,
            message: "No configuration file found".to_string(),
            fix: Some(FixAction {
                description: "Create default configuration".to_string(),
                action: FixType::CreateFile {
                    path: "config.toml".to_string(),
                    content: include_str!("../../../config.example.toml").to_string(),
                },
            }),
        }
    }

    async fn check_database() -> CheckResult {
        let db_path = Path::new("./data/uhorse.db");

        if db_path.exists() {
            CheckResult {
                name: "Database".to_string(),
                status: CheckStatus::Ok,
                message: "Database file exists".to_string(),
                fix: None,
            }
        } else {
            CheckResult {
                name: "Database".to_string(),
                status: CheckStatus::Warning,
                message: "Database file not found (will be created on first run)".to_string(),
                fix: Some(FixAction {
                    description: "Create data directory".to_string(),
                    action: FixType::CreateDir {
                        path: "./data".to_string(),
                    },
                }),
            }
        }
    }

    fn check_llm_key() -> CheckResult {
        if std::env::var("LLM_API_KEY").is_ok()
            || std::env::var("OPENAI_API_KEY").is_ok()
            || std::env::var("ANTHROPIC_API_KEY").is_ok()
        {
            CheckResult {
                name: "LLM API Key".to_string(),
                status: CheckStatus::Ok,
                message: "LLM API key configured".to_string(),
                fix: None,
            }
        } else {
            CheckResult {
                name: "LLM API Key".to_string(),
                status: CheckStatus::Warning,
                message: "No LLM API key found in environment".to_string(),
                fix: Some(FixAction {
                    description: "Set OPENAI_API_KEY or ANTHROPIC_API_KEY environment variable".to_string(),
                    action: FixType::SetEnvVar {
                        name: "OPENAI_API_KEY".to_string(),
                        description: "Get your API key from https://platform.openai.com/api-keys".to_string(),
                    },
                }),
            }
        }
    }

    fn check_data_dir() -> CheckResult {
        let data_dir = Path::new("./data");

        if data_dir.exists() {
            // Check if writable
            let test_file = data_dir.join(".write_test");
            match std::fs::write(&test_file, "test") {
                Ok(_) => {
                    let _ = std::fs::remove_file(&test_file);
                    CheckResult {
                        name: "Data Directory".to_string(),
                        status: CheckStatus::Ok,
                        message: "Data directory is writable".to_string(),
                        fix: None,
                    }
                }
                Err(_) => CheckResult {
                    name: "Data Directory".to_string(),
                    status: CheckStatus::Error,
                    message: "Data directory exists but is not writable".to_string(),
                    fix: Some(FixAction {
                        description: "Fix permissions: chmod 755 ./data".to_string(),
                        action: FixType::Manual {
                            instructions: "Run: chmod 755 ./data".to_string(),
                        },
                    }),
                },
            }
        } else {
            CheckResult {
                name: "Data Directory".to_string(),
                status: CheckStatus::Warning,
                message: "Data directory does not exist".to_string(),
                fix: Some(FixAction {
                    description: "Create data directory".to_string(),
                    action: FixType::CreateDir {
                        path: "./data".to_string(),
                    },
                }),
            }
        }
    }

    fn check_port() -> CheckResult {
        use std::net::TcpListener;

        match TcpListener::bind("127.0.0.1:8080") {
            Ok(_) => CheckResult {
                name: "Port 8080".to_string(),
                status: CheckStatus::Ok,
                message: "Port 8080 is available".to_string(),
                fix: None,
            },
            Err(_) => CheckResult {
                name: "Port 8080".to_string(),
                status: CheckStatus::Warning,
                message: "Port 8080 is in use".to_string(),
                fix: Some(FixAction {
                    description: "Stop conflicting service or change port in config.toml".to_string(),
                    action: FixType::Manual {
                        instructions: "Find process: lsof -i :8080, or change port in config.toml".to_string(),
                    },
                }),
            },
        }
    }

    fn check_deps() -> CheckResult {
        let mut found = Vec::new();

        // Check optional dependencies
        if Command::new("redis-cli").arg("ping").output().is_ok() {
            found.push("Redis");
        }
        if Command::new("nats-server").arg("--version").output().is_ok() {
            found.push("NATS");
        }
        if Command::new("etcdctl").arg("version").output().is_ok() {
            found.push("etcd");
        }

        if found.is_empty() {
            CheckResult {
                name: "Optional Dependencies".to_string(),
                status: CheckStatus::Ok,
                message: "No optional dependencies (Redis, NATS, etcd not required for single-server)".to_string(),
                fix: None,
            }
        } else {
            CheckResult {
                name: "Optional Dependencies".to_string(),
                status: CheckStatus::Ok,
                message: format!("Found: {}", found.join(", ")),
                fix: None,
            }
        }
    }
}
