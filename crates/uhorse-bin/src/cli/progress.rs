//! Progress indicators and spinners
//!
//! Provides progress bars, spinners, and loading animations using indicatif.

use indicatif::{ProgressBar, ProgressStyle, ProgressState};
use std::time::Duration;

/// Create a spinner for indeterminate progress
pub fn spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .expect("Invalid spinner template")
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

/// Create a progress bar for determinate progress
pub fn progress_bar(total: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template(
            "{msg:.cyan} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
        )
        .expect("Invalid progress template")
        .with_key("eta", |state: &ProgressState, w: &mut dyn std::fmt::Write| {
            write!(w, "{:.1}s", state.eta().as_secs_f64()).expect("Write failed")
        })
        .progress_chars("━╸ "),
    );
    pb.set_message(message.to_string());
    pb
}

/// Create a download-style progress bar with bytes
pub fn download_progress(total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
        )
        .expect("Invalid download template")
        .with_key("eta", |state: &ProgressState, w: &mut dyn std::fmt::Write| {
            write!(w, "{:.1}s", state.eta().as_secs_f64()).expect("Write failed")
        })
        .progress_chars("━╸ "),
    );
    pb
}

/// Show a simple loading animation with checkmark on completion
pub struct LoadingIndicator {
    pb: ProgressBar,
}

impl LoadingIndicator {
    /// Create a new loading indicator
    pub fn new(message: &str) -> Self {
        Self {
            pb: spinner(message),
        }
    }

    /// Update the message
    pub fn set_message(&self, message: &str) {
        self.pb.set_message(message.to_string());
    }

    /// Complete with success
    pub fn success(self, message: &str) {
        self.pb.finish_with_message(format!("✓ {}", message));
    }

    /// Complete with error
    pub fn error(self, message: &str) {
        self.pb.finish_with_message(format!("✗ {}", message));
    }

    /// Complete with warning
    pub fn warning(self, message: &str) {
        self.pb.finish_with_message(format!("⚠ {}", message));
    }
}

/// Multi-step progress tracker
pub struct StepsProgress {
    pb: ProgressBar,
    current: usize,
    total: usize,
}

impl StepsProgress {
    /// Create a new steps progress tracker
    pub fn new(total: usize, title: &str) -> Self {
        let pb = ProgressBar::new(total as u64);
        pb.set_style(
            ProgressStyle::with_template(
                &format!("{{msg}} [{{bar:40.cyan/blue}}] {{pos}}/{} steps", total),
            )
            .expect("Invalid steps template")
            .progress_chars("━╸ "),
        );
        pb.set_message(title.to_string());
        Self {
            pb,
            current: 0,
            total,
        }
    }

    /// Advance to next step
    pub fn next(&mut self, step_name: &str) {
        self.current += 1;
        self.pb.set_message(step_name.to_string());
        self.pb.inc(1);
    }

    /// Complete all steps
    pub fn finish(self, message: &str) {
        self.pb.finish_with_message(message.to_string());
    }
}

/// Run a closure with a loading spinner
pub fn with_spinner<T, E, F>(message: &str, success_msg: &str, f: F) -> Result<T, E>
where
    F: FnOnce() -> Result<T, E>,
{
    let pb = spinner(message);
    let result = f();
    match &result {
        Ok(_) => pb.finish_with_message(format!("✓ {}", success_msg)),
        Err(_) => pb.finish_with_message(format!("✗ Failed: {}", message)),
    }
    result
}

/// Run a closure with a progress bar
pub fn with_progress<T, E, F>(total: u64, message: &str, f: F) -> Result<T, E>
where
    F: FnOnce(&ProgressBar) -> Result<T, E>,
{
    let pb = progress_bar(total, message);
    let result = f(&pb);
    if result.is_ok() {
        pb.finish();
    } else {
        pb.abandon();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_spinner() {
        let pb = spinner("Loading...");
        thread::sleep(Duration::from_millis(200));
        pb.finish_with_message("Done");
    }

    #[test]
    fn test_progress_bar() {
        let pb = progress_bar(100, "Processing");
        for _ in 0..100 {
            pb.inc(1);
            thread::sleep(Duration::from_millis(10));
        }
        pb.finish();
    }

    #[test]
    fn test_loading_indicator() {
        let loader = LoadingIndicator::new("Installing dependencies...");
        thread::sleep(Duration::from_millis(500));
        loader.success("Dependencies installed");
    }

    #[test]
    fn test_steps_progress() {
        let mut steps = StepsProgress::new(3, "Setup");
        thread::sleep(Duration::from_millis(100));
        steps.next("Step 1 complete");
        thread::sleep(Duration::from_millis(100));
        steps.next("Step 2 complete");
        thread::sleep(Duration::from_millis(100));
        steps.next("Step 3 complete");
        steps.finish("Setup complete");
    }
}
