use std::borrow::Cow;
use std::fmt::Write as _;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

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

/// Calculates the visible display width of a string by stripping ANSI escape sequences.
pub fn visible_width(s: &str) -> usize {
    let mut width = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else {
            width += 1;
        }
    }
    width
}

/// Formats a list of content lines into a rounded box card with a title.
pub fn draw_box(title: &str, content_lines: &[String]) -> String {
    let title_chars = title.chars().count();
    let content_width = content_lines
        .iter()
        .map(|l| visible_width(l))
        .max()
        .unwrap_or(36)
        .max(title_chars + 4)
        .max(46);

    let inner_width = content_width + 2;
    let remaining = inner_width.saturating_sub(title_chars + 3);

    // Box-drawing characters ('─', '│', corners) are 3 bytes each in UTF-8.
    // Account for 3-byte horizontal borders plus line text, padding, and ANSI codes.
    let border_bytes = (inner_width * 3 + 4) * 2;
    let content_bytes = content_lines.iter().map(|l| l.len() + 16).sum::<usize>();
    let est_size = border_bytes + content_bytes;
    let mut out = String::with_capacity(est_size);

    // Top border: ╭─ Title ──────...──╮
    let _ = writeln!(out, "╭─ {title} {:─<remaining$}╮", "");

    // Content lines: │ Content... │
    for line in content_lines {
        let pad = content_width.saturating_sub(visible_width(line));
        let _ = writeln!(out, "│ {line}{:pad$} │", "");
    }

    // Bottom border: ╰────────...──╯
    let _ = writeln!(out, "╰{:─<inner_width$}╯", "");

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

    #[test]
    fn test_draw_box_ansi_alignment() {
        let lines = vec![
            "Plain text line".to_string(),
            "\x1b[32m✔\x1b[0m - Option with green check".to_string(),
            "\x1b[31m✖\x1b[0m - Option with red cross".to_string(),
        ];
        let card = draw_box("ANSI Test", &lines);
        let card_lines: Vec<&str> = card.lines().collect();
        assert_eq!(card_lines.len(), 5);
        let expected_visible_width = visible_width(card_lines[0]);
        for line in &card_lines {
            assert_eq!(visible_width(line), expected_visible_width);
        }
    }
}
