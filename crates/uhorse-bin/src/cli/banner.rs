//! ASCII art banner and splash screen
//!
//! Provides visual branding for the CLI.

use colored::*;

/// Print the uHorse ASCII art banner
pub fn print_banner() {
    let banner = r#"

   ██╗   ██╗██╗ ██████╗ ██████╗ ███████╗
   ██║   ██║██║██╔════╝██╔═══██╗██╔════╝
   ██║   ██║██║██║     ██║   ██║███████╗
   ╚██╗ ██╔╝██║██║     ██║   ██║╚════██║
    ╚████╔╝ ██║╚██████╗╚██████╔╝███████║
     ╚═══╝  ╚═╝ ╚═════╝ ╚═════╝ ╚══════╝

"#;
    println!("{}", banner.cyan().bold());
}

/// Print the uHorse banner with version
pub fn print_banner_with_version(version: &str) {
    print_banner();
    println!(
        "{} {}",
        "Enterprise AI Infrastructure Platform".dimmed(),
        format!("v{}", version).green().bold()
    );
    println!();
}

/// Print the startup splash screen
pub fn print_splash(version: &str) {
    println!();
    print_banner_with_version(version);

    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    println!();
    println!("  {} Multi-channel AI Gateway", "→".cyan());
    println!("  {} Agent Orchestration", "→".cyan());
    println!("  {} Skill System", "→".cyan());
    println!("  {} Memory Management", "→".cyan());
    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed());
    println!();
}

/// Print a compact one-line banner
pub fn print_compact_banner(version: &str) {
    println!(
        "{} {} {}",
        "🦄".purple(),
        "uHorse".cyan().bold(),
        format!("v{}", version).dimmed()
    );
}

/// Print welcome message for interactive mode
pub fn print_welcome() {
    println!();
    println!("{}", "╔══════════════════════════════════════════════════════════════════════╗".cyan());
    println!("{}", "║                                                                      ║".cyan());
    println!(
        "{}",
        format!("║{:^70}║", "Welcome to uHorse Configuration Wizard").cyan()
    );
    println!("{}", "║                                                                      ║".cyan());
    println!("{}", "╚══════════════════════════════════════════════════════════════════════╝".cyan());
    println!();
    println!("  This wizard will help you set up your uHorse instance.");
    println!("  Press {} at any time to skip optional questions.\n", "Enter".yellow());
}

/// Print completion message
pub fn print_completion(config_path: &str) {
    println!();
    println!("{}", "╔══════════════════════════════════════════════════════════════════════╗".green());
    println!("{}", "║                    ✓ Configuration Complete!                         ║".green());
    println!("{}", "╚══════════════════════════════════════════════════════════════════════╝".green());
    println!();
    println!("  Configuration saved to: {}", config_path.green());
    println!();
    println!("  Next steps:");
    println!("    {} Start the server:", "1.".cyan());
    println!("      {}", "uhorse start".yellow());
    println!();
    println!("    {} Check health:", "2.".cyan());
    println!("      {}", "curl http://localhost:8080/health/live".yellow());
    println!();
    println!("    {} View docs:", "3.".cyan());
    println!("      {}", "https://uhorse.ai/docs".yellow());
    println!();
}

/// Print error banner
pub fn print_error_banner(title: &str, message: &str) {
    println!();
    println!("{}", "╔══════════════════════════════════════════════════════════════════════╗".red());
    println!("{}", format!("║  ✗ {:^66}║", title).red());
    println!("{}", "╚══════════════════════════════════════════════════════════════════════╝".red());
    println!();
    println!("  {}", message);
    println!();
}

/// Print warning banner
pub fn print_warning_banner(title: &str, message: &str) {
    println!();
    println!("{}", "╔══════════════════════════════════════════════════════════════════════╗".yellow());
    println!("{}", format!("║  ⚠ {:^66}║", title).yellow());
    println!("{}", "╚══════════════════════════════════════════════════════════════════════╝".yellow());
    println!();
    println!("  {}", message);
    println!();
}

/// Print success banner
pub fn print_success_banner(title: &str, message: &str) {
    println!();
    println!("{}", "╔══════════════════════════════════════════════════════════════════════╗".green());
    println!("{}", format!("║  ✓ {:^66}║", title).green());
    println!("{}", "╚══════════════════════════════════════════════════════════════════════╝".green());
    println!();
    if !message.is_empty() {
        println!("  {}", message);
        println!();
    }
}

/// Print info box
pub fn print_info_box(title: &str, content: &[&str]) {
    println!();
    println!("{}", "┌──────────────────────────────────────────────────────────────────────┐".cyan());
    println!("{}", format!("│ {}: {:^60}│", "ℹ".blue(), title).cyan());
    println!("{}", "├──────────────────────────────────────────────────────────────────────┤".cyan());
    for line in content {
        println!("{}", format!("│ {:<68}│", line).cyan());
    }
    println!("{}", "└──────────────────────────────────────────────────────────────────────┘".cyan());
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_banner() {
        print_banner();
    }

    #[test]
    fn test_print_banner_with_version() {
        print_banner_with_version("3.5.0");
    }

    #[test]
    fn test_print_compact_banner() {
        print_compact_banner("3.5.0");
    }

    #[test]
    fn test_print_info_box() {
        print_info_box(
            "Available Channels",
            &[
                "• Telegram - Full Bot API support",
                "• DingTalk - Enterprise messaging",
                "• Feishu - Rich card messages",
            ],
        );
    }
}
