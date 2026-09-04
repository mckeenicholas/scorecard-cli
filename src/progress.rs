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

/// Formats a list of content lines into a rounded box card with a title.
pub fn draw_box(title: &str, content_lines: &[String]) -> String {
    let (tl, tr, bl, br, h, v) = ('╭', '╮', '╰', '╯', '─', '│');

    let content_width = content_lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(36)
        .max(title.chars().count() + 4)
        .max(46);

    let inner_width = content_width + 2;

    let mut out = String::new();

    // Top border: ╭─ Title ──────...──╮
    out.push(tl);
    out.push(h);
    out.push(' ');
    out.push_str(title);
    out.push(' ');
    let remaining = inner_width.saturating_sub(title.chars().count() + 3);
    for _ in 0..remaining {
        out.push(h);
    }
    out.push(tr);
    out.push('\n');

    // Content lines
    for line in content_lines {
        out.push(v);
        out.push(' ');
        out.push_str(line);
        let pad = content_width.saturating_sub(line.chars().count());
        for _ in 0..pad {
            out.push(' ');
        }
        out.push(' ');
        out.push(v);
        out.push('\n');
    }

    // Bottom border: ╰────────...──╯
    out.push(bl);
    for _ in 0..inner_width {
        out.push(h);
    }
    out.push(br);
    out.push('\n');

    out
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

    #[test]
    fn test_draw_box() {
        let lines = vec!["Item 1: Hello".to_string(), "Item 2: World".to_string()];
        let card = draw_box("Header", &lines);
        assert!(card.contains("╭─ Header"));
        assert!(card.contains("│ Item 1: Hello"));
        assert!(card.contains('╰'));
    }
}
