//! Colored output utilities
//!
//! Provides styled console output with colors, icons, and formatting.

use colored::*;
use std::fmt::Display;

/// Output style configuration
pub struct OutputStyle {
    pub use_colors: bool,
    pub use_icons: bool,
}

impl Default for OutputStyle {
    fn default() -> Self {
        Self {
            use_colors: true,
            use_icons: true,
        }
    }
}

/// Print a success message with green checkmark
pub fn success<S: AsRef<str>>(msg: S) {
    let style = OutputStyle::default();
    if style.use_icons && style.use_colors {
        println!("{} {}", "✓".green().bold(), msg.as_ref());
    } else if style.use_icons {
        println!("✓ {}", msg.as_ref());
    } else {
        println!("[SUCCESS] {}", msg.as_ref());
    }
}

/// Print an error message with red X
pub fn error<S: AsRef<str>>(msg: S) {
    let style = OutputStyle::default();
    if style.use_icons && style.use_colors {
        eprintln!("{} {}", "✗".red().bold(), msg.as_ref());
    } else if style.use_icons {
        eprintln!("✗ {}", msg.as_ref());
    } else {
        eprintln!("[ERROR] {}", msg.as_ref());
    }
}

/// Print a warning message with yellow triangle
pub fn warning<S: AsRef<str>>(msg: S) {
    let style = OutputStyle::default();
    if style.use_icons && style.use_colors {
        println!("{} {}", "⚠".yellow().bold(), msg.as_ref());
    } else if style.use_icons {
        println!("⚠ {}", msg.as_ref());
    } else {
        println!("[WARNING] {}", msg.as_ref());
    }
}

/// Print an info message with blue circle
pub fn info<S: AsRef<str>>(msg: S) {
    let style = OutputStyle::default();
    if style.use_icons && style.use_colors {
        println!("{} {}", "ℹ".blue().bold(), msg.as_ref());
    } else if style.use_icons {
        println!("ℹ {}", msg.as_ref());
    } else {
        println!("[INFO] {}", msg.as_ref());
    }
}

/// Print a step message with arrow
pub fn step<S: AsRef<str>>(msg: S) {
    let style = OutputStyle::default();
    if style.use_colors {
        println!("{} {}", "→".cyan(), msg.as_ref());
    } else {
        println!("-> {}", msg.as_ref());
    }
}

/// Print a header/title
pub fn header<S: AsRef<str>>(msg: S) {
    let style = OutputStyle::default();
    if style.use_colors {
        println!();
        println!("{}", msg.as_ref().bold().underline());
        println!();
    } else {
        println!();
        println!("{}", msg.as_ref());
        println!();
    }
}

/// Print a subheader
pub fn subheader<S: AsRef<str>>(msg: S) {
    let style = OutputStyle::default();
    if style.use_colors {
        println!("\n{}", msg.as_ref().bold().cyan());
    } else {
        println!("\n{}", msg.as_ref());
    }
}

/// Print a bullet point item
pub fn bullet<S: AsRef<str>>(msg: S) {
    println!("  • {}", msg.as_ref());
}

/// Print a key-value pair
pub fn kv<K: Display, V: Display>(key: K, value: V) {
    let style = OutputStyle::default();
    if style.use_colors {
        println!("  {}: {}", key.to_string().dimmed(), value);
    } else {
        println!("  {}: {}", key, value);
    }
}

/// Print a status line with label
pub fn status<S: AsRef<str>>(label: S, value: S) {
    let style = OutputStyle::default();
    if style.use_colors {
        println!(
            "  {:<20} {}",
            format!("{}:", label.as_ref()).dimmed(),
            value.as_ref().green()
        );
    } else {
        println!("  {:<20} {}", format!("{}:", label.as_ref()), value.as_ref());
    }
}

/// Print a separator line
pub fn separator() {
    let style = OutputStyle::default();
    if style.use_colors {
        println!("{}", "─".repeat(60).dimmed());
    } else {
        println!("{}", "─".repeat(60));
    }
}

/// Print an empty line
pub fn newline() {
    println!();
}

/// Print a debug message (only in debug mode)
pub fn debug<S: AsRef<str>>(msg: S) {
    #[cfg(debug_assertions)]
    {
        let style = OutputStyle::default();
        if style.use_colors {
            println!("{} {}", "🐛".purple(), msg.as_ref().dimmed());
        } else {
            println!("[DEBUG] {}", msg.as_ref());
        }
    }
}

/// Highlight text with color
pub fn highlight<S: AsRef<str>>(text: S) -> String {
    let style = OutputStyle::default();
    if style.use_colors {
        text.as_ref().yellow().bold().to_string()
    } else {
        text.as_ref().to_string()
    }
}

/// Dim text
pub fn dim<S: AsRef<str>>(text: S) -> String {
    let style = OutputStyle::default();
    if style.use_colors {
        text.as_ref().dimmed().to_string()
    } else {
        text.as_ref().to_string()
    }
}

/// Color text green
pub fn green<S: AsRef<str>>(text: S) -> String {
    let style = OutputStyle::default();
    if style.use_colors {
        text.as_ref().green().to_string()
    } else {
        text.as_ref().to_string()
    }
}

/// Color text red
pub fn red<S: AsRef<str>>(text: S) -> String {
    let style = OutputStyle::default();
    if style.use_colors {
        text.as_ref().red().to_string()
    } else {
        text.as_ref().to_string()
    }
}

/// Color text cyan
pub fn cyan<S: AsRef<str>>(text: S) -> String {
    let style = OutputStyle::default();
    if style.use_colors {
        text.as_ref().cyan().to_string()
    } else {
        text.as_ref().to_string()
    }
}

/// Color text yellow
pub fn yellow<S: AsRef<str>>(text: S) -> String {
    let style = OutputStyle::default();
    if style.use_colors {
        text.as_ref().yellow().to_string()
    } else {
        text.as_ref().to_string()
    }
}

/// Print a table row
pub fn table_row(columns: &[&str], widths: &[usize]) {
    let formatted: Vec<String> = columns
        .iter()
        .zip(widths.iter())
        .map(|(col, &width)| format!("{:width$}", col, width = width))
        .collect();
    println!("  {}", formatted.join(" "));
}

/// Print a table header
pub fn table_header(columns: &[&str], widths: &[usize]) {
    let style = OutputStyle::default();
    let formatted: Vec<String> = columns
        .iter()
        .zip(widths.iter())
        .map(|(col, &width)| {
            if style.use_colors {
                format!("{:width$}", col.bold(), width = width)
            } else {
                format!("{:width$}", col, width = width)
            }
        })
        .collect();
    println!("  {}", formatted.join(" "));
    separator();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_functions() {
        success("Operation completed successfully");
        warning("This is a warning");
        info("This is informational");
        step("Processing step 1");
        header("Main Header");
        bullet("List item");
        kv("Key", "Value");
    }
}
