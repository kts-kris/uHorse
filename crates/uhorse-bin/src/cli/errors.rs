//! Error codes with actionable suggestions
//!
//! Provides structured error handling with codes, causes, and solutions.

use std::fmt;

/// Error categories
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Configuration errors (E1xxx)
    Config,
    /// Database errors (E2xxx)
    Database,
    /// Network/API errors (E3xxx)
    Network,
    /// Authentication errors (E4xxx)
    Auth,
    /// Validation errors (E5xxx)
    Validation,
    /// System errors (E6xxx)
    System,
    /// Channel errors (E7xxx)
    Channel,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorCategory::Config => write!(f, "Configuration"),
            ErrorCategory::Database => write!(f, "Database"),
            ErrorCategory::Network => write!(f, "Network"),
            ErrorCategory::Auth => write!(f, "Authentication"),
            ErrorCategory::Validation => write!(f, "Validation"),
            ErrorCategory::System => write!(f, "System"),
            ErrorCategory::Channel => write!(f, "Channel"),
        }
    }
}

/// Structured error with code and suggestions
#[derive(Debug, Clone)]
pub struct UHorseError {
    /// Error code (e.g., E1001)
    pub code: &'static str,
    /// Error category
    pub category: ErrorCategory,
    /// Short description
    pub message: String,
    /// Possible causes
    pub causes: Vec<&'static str>,
    /// Suggested solutions
    pub solutions: Vec<&'static str>,
    /// Link to documentation
    pub doc_link: Option<&'static str>,
}

impl UHorseError {
    /// Create a new error
    pub fn new(
        code: &'static str,
        category: ErrorCategory,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            category,
            message: message.into(),
            causes: Vec::new(),
            solutions: Vec::new(),
            doc_link: None,
        }
    }

    /// Add a possible cause
    pub fn cause(mut self, cause: &'static str) -> Self {
        self.causes.push(cause);
        self
    }

    /// Add a suggested solution
    pub fn solution(mut self, solution: &'static str) -> Self {
        self.solutions.push(solution);
        self
    }

    /// Add documentation link
    pub fn doc(mut self, link: &'static str) -> Self {
        self.doc_link = Some(link);
        self
    }

    /// Print the error with full context
    pub fn print(&self) {
        use colored::*;

        eprintln!();
        eprintln!(
            "{} [{}] {}",
            "ERROR".red().bold(),
            self.code.yellow(),
            self.message
        );
        eprintln!("{}", "━".repeat(60).dimmed());

        if !self.causes.is_empty() {
            eprintln!("\n{} Possible causes:", "?".yellow());
            for cause in &self.causes {
                eprintln!("  • {}", cause);
            }
        }

        if !self.solutions.is_empty() {
            eprintln!("\n{} Suggested solutions:", "→".cyan());
            for (i, solution) in self.solutions.iter().enumerate() {
                eprintln!("  {}. {}", i + 1, solution);
            }
        }

        if let Some(link) = self.doc_link {
            eprintln!("\n{} Documentation: {}", "📖".blue(), link.cyan());
        }

        eprintln!();
    }
}

impl fmt::Display for UHorseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for UHorseError {}

// ============================================================================
// Predefined Errors
// ============================================================================

/// Configuration file not found
pub fn err_config_not_found() -> UHorseError {
    UHorseError::new(
        "E1001",
        ErrorCategory::Config,
        "Configuration file not found",
    )
    .cause("No config.toml in current directory")
    .cause("No ~/.uhorse/config.toml exists")
    .solution("Run 'uhorse wizard' to create a configuration")
    .solution("Copy config.example.toml to config.toml and edit")
    .solution("Specify config path with --config flag")
    .doc("https://uhorse.ai/docs/configuration")
}

/// Invalid configuration
pub fn err_config_invalid(field: &str) -> UHorseError {
    UHorseError::new(
        "E1002",
        ErrorCategory::Config,
        format!("Invalid configuration: {}", field),
    )
    .cause("Missing required field")
    .cause("Invalid value format")
    .cause("Type mismatch in configuration value")
    .solution("Check config.toml syntax with a TOML validator")
    .solution("Refer to config.example.toml for correct format")
    .solution("Run 'uhorse doctor' to diagnose configuration issues")
    .doc("https://uhorse.ai/docs/configuration")
}

/// Database connection failed
pub fn err_database_connection() -> UHorseError {
    UHorseError::new(
        "E2001",
        ErrorCategory::Database,
        "Failed to connect to database",
    )
    .cause("Database file does not exist")
    .cause("Insufficient permissions to access database")
    .cause("Database is locked by another process")
    .cause("Invalid connection string")
    .solution("Check database path in config.toml")
    .solution("Ensure data directory exists and is writable")
    .solution("Stop other uHorse instances using the same database")
    .solution("Run 'uhorse doctor' to check database connectivity")
    .doc("https://uhorse.ai/docs/database")
}

/// Database migration failed
pub fn err_database_migration() -> UHorseError {
    UHorseError::new(
        "E2002",
        ErrorCategory::Database,
        "Database migration failed",
    )
    .cause("Schema version mismatch")
    .cause("Corrupted database file")
    .cause("Insufficient disk space")
    .solution("Backup database and run 'uhorse migrate --reset'")
    .solution("Check database integrity with SQLite tools")
    .solution("Contact support with database file for recovery")
    .doc("https://uhorse.ai/docs/migrations")
}

/// LLM API error
pub fn err_llm_api(provider: &str) -> UHorseError {
    UHorseError::new(
        "E3001",
        ErrorCategory::Network,
        format!("LLM API error: {}", provider),
    )
    .cause("Invalid API key")
    .cause("API quota exceeded")
    .cause("Network connectivity issue")
    .cause("API endpoint is unreachable")
    .solution("Verify API key in config.toml or LLM_API_KEY env var")
    .solution("Check API usage limits and billing")
    .solution("Test connectivity: curl https://api.openai.com/v1/models")
    .solution("Try alternative LLM provider")
    .doc("https://uhorse.ai/docs/llm-providers")
}

/// Authentication failed
pub fn err_auth_failed() -> UHorseError {
    UHorseError::new(
        "E4001",
        ErrorCategory::Auth,
        "Authentication failed",
    )
    .cause("Invalid credentials")
    .cause("Token expired")
    .cause("User not found")
    .solution("Check username and password")
    .solution("Request a new access token")
    .solution("Verify user exists in the system")
    .doc("https://uhorse.ai/docs/authentication")
}

/// Token expired
pub fn err_token_expired() -> UHorseError {
    UHorseError::new(
        "E4002",
        ErrorCategory::Auth,
        "Access token expired",
    )
    .cause("Token TTL exceeded")
    .solution("Refresh the token using refresh token")
    .solution("Re-authenticate to get new tokens")
    .doc("https://uhorse.ai/docs/authentication")
}

/// Invalid input validation
pub fn err_validation_failed(field: &str, reason: &str) -> UHorseError {
    UHorseError::new(
        "E5001",
        ErrorCategory::Validation,
        format!("Validation failed for '{}': {}", field, reason),
    )
    .cause("Input format is incorrect")
    .cause("Value out of acceptable range")
    .cause("Required field is empty")
    .solution("Check input format and try again")
    .solution("Refer to API documentation for valid values")
    .doc("https://uhorse.ai/docs/api/validation")
}

/// Port already in use
pub fn err_port_in_use(port: u16) -> UHorseError {
    UHorseError::new(
        "E6001",
        ErrorCategory::System,
        format!("Port {} is already in use", port),
    )
    .cause("Another uHorse instance is running")
    .cause("Different service is using this port")
    .solution("Stop existing uHorse: pkill uhorse")
    .solution("Change port in config.toml")
    .solution("Find process using port: lsof -i :<port>")
    .doc("https://uhorse.ai/docs/deployment")
}

/// Insufficient permissions
pub fn err_permission_denied(path: &str) -> UHorseError {
    UHorseError::new(
        "E6002",
        ErrorCategory::System,
        format!("Permission denied: {}", path),
    )
    .cause("Insufficient file system permissions")
    .cause("Running as non-root user for privileged operation")
    .solution("Check file/directory permissions")
    .solution("Run with appropriate permissions")
    .solution("Change ownership: chown -R user:group ./data")
    .doc("https://uhorse.ai/docs/deployment")
}

/// Channel connection failed
pub fn err_channel_connection(channel: &str) -> UHorseError {
    UHorseError::new(
        "E7001",
        ErrorCategory::Channel,
        format!("Failed to connect to {}: connection refused", channel),
    )
    .cause("Invalid bot token or credentials")
    .cause("Network connectivity issue")
    .cause("Channel API is unavailable")
    .cause("Webhook URL is not accessible")
    .solution("Verify bot token in config.toml")
    .solution("Check network connectivity to channel API")
    .solution("Ensure webhook URL is publicly accessible")
    .solution("Test credentials with channel's API explorer")
    .doc("https://uhorse.ai/docs/channels")
}

/// Channel rate limited
pub fn err_channel_rate_limited(channel: &str) -> UHorseError {
    UHorseError::new(
        "E7002",
        ErrorCategory::Channel,
        format!("{} API rate limit exceeded", channel),
    )
    .cause("Too many requests in short time")
    .cause("API quota exceeded")
    .solution("Wait and retry (exponential backoff)")
    .solution("Reduce request frequency")
    .solution("Contact channel for quota increase")
    .doc("https://uhorse.ai/docs/channels/rate-limits")
}

/// Skill execution failed
pub fn err_skill_execution(skill: &str, reason: &str) -> UHorseError {
    UHorseError::new(
        "E5002",
        ErrorCategory::Validation,
        format!("Skill '{}' execution failed: {}", skill, reason),
    )
    .cause("Invalid skill parameters")
    .cause("Skill timeout")
    .cause("Missing dependencies")
    .solution("Check skill parameters match schema")
    .solution("Increase timeout in config")
    .solution("Verify skill dependencies are installed")
    .doc("https://uhorse.ai/docs/skills")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_print() {
        err_config_not_found().print();
    }

    #[test]
    fn test_error_with_field() {
        err_config_invalid("server.port").print();
    }

    #[test]
    fn test_database_error() {
        err_database_connection().print();
    }

    #[test]
    fn test_channel_error() {
        err_channel_connection("Telegram").print();
    }
}
