use indicatif::{ProgressBar, ProgressStyle};
use std::borrow::Cow;
use std::time::Duration;

/// Tick interval for loading spinners (80ms creates a smooth ~12.5 FPS animation).
pub const SPINNER_TICK_INTERVAL: Duration = Duration::from_millis(80);

/// Braille spinner characters for clean, modern terminal animation.
pub const SPINNER_CHARS: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";

/// Creates and starts an animated loading ticker/spinner with custom message.
pub fn create_spinner(message: impl Into<Cow<'static, str>>) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_chars(SPINNER_CHARS),
    );
    spinner.set_message(message);
    spinner.enable_steady_tick(SPINNER_TICK_INTERVAL);
    spinner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_spinner() {
        let spinner = create_spinner("Testing spinner operation...");
        assert!(!spinner.is_finished());
        spinner.set_message("Updating message...");
        spinner.finish_and_clear();
        assert!(spinner.is_finished());
    }
}
