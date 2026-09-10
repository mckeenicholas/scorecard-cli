use super::suggest::{fetch_wca_competitions, get_local_json_suggestions};
use super::types::{InteractiveError, RawModeGuard, Suggestion};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::{self, Stylize},
    terminal::{self, ClearType},
};
use std::io::{Write, stdout};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

pub struct SharedSearchState {
    pub is_loading: bool,
    pub api_cache: std::collections::HashMap<String, Vec<Suggestion>>,
    pub api_results: Vec<Suggestion>,
    pub api_version: usize,
}

/// Helper function to clear previous rendered lines from row 0 downwards and return to row 0.
pub fn clear_widget_lines<W: Write>(out: &mut W, lines_count: usize) -> Result<(), std::io::Error> {
    if lines_count == 0 {
        return Ok(());
    }
    queue!(out, cursor::MoveToColumn(0))?;
    for i in 0..lines_count {
        queue!(out, terminal::Clear(ClearType::CurrentLine))?;
        if i + 1 < lines_count {
            queue!(out, cursor::MoveDown(1))?;
        }
    }
    if lines_count > 1 {
        let up = u16::try_from(lines_count - 1).unwrap_or(u16::MAX);
        queue!(out, cursor::MoveUp(up))?;
    }
    queue!(out, cursor::MoveToColumn(0))?;
    Ok(())
}

pub fn spawn_search_worker(rx: mpsc::Receiver<String>, shared: Arc<Mutex<SharedSearchState>>) {
    std::thread::spawn(move || {
        while let Ok(mut current_query) = rx.recv() {
            // Debounce loop: wait up to 250ms for newer keystrokes
            while let Ok(newer_query) = rx.recv_timeout(Duration::from_millis(250)) {
                current_query = newer_query;
            }

            let trimmed = current_query.trim().to_string();
            let is_path_query = trimmed.starts_with('~')
                || trimmed.starts_with('/')
                || trimmed.starts_with('\\')
                || trimmed.starts_with('.')
                || trimmed.contains('/')
                || trimmed.contains('\\');

            if is_path_query || trimmed.len() < 3 {
                if let Ok(mut state) = shared.lock() {
                    state.api_results.clear();
                    state.is_loading = false;
                    state.api_version += 1;
                }
                continue;
            }

            let already_cached = {
                if let Ok(mut state) = shared.lock() {
                    let SharedSearchState {
                        api_cache,
                        api_results,
                        is_loading,
                        api_version,
                    } = &mut *state;
                    if let Some(cached) = api_cache.get(&trimmed) {
                        api_results.clone_from(cached);
                        *is_loading = false;
                        *api_version += 1;
                        true
                    } else {
                        *is_loading = true;
                        *api_version += 1;
                        false
                    }
                } else {
                    false
                }
            };

            if already_cached {
                continue;
            }

            let results = fetch_wca_competitions(&trimmed).unwrap_or_default();

            if let Ok(mut state) = shared.lock() {
                state.api_cache.insert(trimmed, results.clone());
                state.api_results = results;
                state.is_loading = false;
                state.api_version += 1;
            }
        }
    });
}

pub fn render_search_widget<W: Write>(
    out: &mut W,
    input_buffer: &str,
    suggestions: &[Suggestion],
    selected_index: Option<usize>,
    is_loading: bool,
    previous_rendered_lines: usize,
) -> Result<usize, std::io::Error> {
    clear_widget_lines(out, previous_rendered_lines)?;

    let mut lines_rendered: usize = 0;
    let searching_indicator = if is_loading {
        " (searching WCA...)"
    } else {
        ""
    };
    queue!(
        out,
        style::SetForegroundColor(style::Color::Cyan),
        style::Print("? "),
        style::ResetColor,
        style::Print("Competition ID or WCIF file path: ".bold()),
        style::Print(input_buffer),
        style::SetForegroundColor(style::Color::DarkGrey),
        style::Print(searching_indicator),
        style::ResetColor,
        style::Print("\r\n"),
    )?;
    lines_rendered += 1;

    // Render suggestions (up to 6 visible)
    let max_visible = 6;
    let total = suggestions.len();
    let (start_idx, end_idx) = if total <= max_visible {
        (0, total)
    } else {
        let cur = selected_index.unwrap_or(0);
        let start = if cur >= max_visible {
            cur - max_visible + 1
        } else {
            0
        };
        let end = (start + max_visible).min(total);
        (start, end)
    };

    for (i, item) in suggestions.iter().enumerate().take(end_idx).skip(start_idx) {
        if selected_index == Some(i) {
            queue!(
                out,
                style::SetForegroundColor(style::Color::Cyan),
                style::Print("  > "),
                style::Print(&item.display),
                style::Print("\r\n"),
                style::ResetColor,
            )?;
        } else {
            queue!(
                out,
                style::Print("    "),
                style::Print(&item.display),
                style::Print("\r\n"),
            )?;
        }
        lines_rendered += 1;
    }

    queue!(
        out,
        style::SetForegroundColor(style::Color::DarkGrey),
        style::Print("  [↑/↓ to navigate, Tab to complete, Enter to select]"),
        style::ResetColor,
    )?;
    lines_rendered += 1;

    let prompt_prefix = "? Competition ID or WCIF file path: ";
    let col = u16::try_from(prompt_prefix.len() + input_buffer.len()).unwrap_or(u16::MAX);
    let rows_to_move_up = u16::try_from(lines_rendered.saturating_sub(1)).unwrap_or(0);
    queue!(
        out,
        cursor::MoveUp(rows_to_move_up),
        cursor::MoveToColumn(col),
        cursor::Show,
    )?;
    out.flush()?;
    Ok(lines_rendered)
}

pub struct SearchInputState<'a> {
    pub input_buffer: &'a mut String,
    pub selected_index: &'a mut Option<usize>,
    pub needs_render: &'a mut bool,
    pub suggestions: &'a [Suggestion],
    pub tx: &'a mpsc::Sender<String>,
}

pub fn handle_search_key(
    state: &mut SearchInputState<'_>,
    key: KeyEvent,
    out: &mut std::io::Stdout,
    previous_rendered_lines: usize,
) -> Result<Option<String>, InteractiveError> {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Err(InteractiveError::Aborted)
        }
        KeyCode::Esc => Err(InteractiveError::Aborted),
        KeyCode::Enter => {
            let chosen = if let Some(idx) = *state.selected_index {
                state.suggestions[idx].value.clone()
            } else {
                state.input_buffer.trim().to_string()
            };

            if chosen.is_empty() {
                return Ok(None);
            }

            if chosen.ends_with('/') || chosen.ends_with('\\') {
                *state.input_buffer = chosen;
                *state.selected_index = None;
                let _ = state.tx.send(state.input_buffer.clone());
                *state.needs_render = true;
                return Ok(None);
            }

            clear_widget_lines(out, previous_rendered_lines)?;
            queue!(
                out,
                style::SetForegroundColor(style::Color::Green),
                style::Print("✔ "),
                style::ResetColor,
                style::Print("Competition ID or WCIF file path: ".bold()),
                style::SetForegroundColor(style::Color::Cyan),
                style::Print(&chosen),
                style::ResetColor,
                style::Print("\r\n"),
            )?;
            out.flush()?;
            Ok(Some(chosen))
        }
        KeyCode::Down => {
            if !state.suggestions.is_empty() {
                *state.selected_index = Some(match *state.selected_index {
                    None => 0,
                    Some(i) => (i + 1).min(state.suggestions.len() - 1),
                });
                *state.needs_render = true;
            }
            Ok(None)
        }
        KeyCode::Up => {
            *state.selected_index = match *state.selected_index {
                None | Some(0) => None,
                Some(i) => Some(i - 1),
            };
            *state.needs_render = true;
            Ok(None)
        }
        KeyCode::Tab => {
            if let Some(idx) = *state.selected_index {
                state.input_buffer.clone_from(&state.suggestions[idx].value);
                *state.selected_index = None;
                let _ = state.tx.send(state.input_buffer.clone());
                *state.needs_render = true;
            } else if !state.suggestions.is_empty() {
                state.input_buffer.clone_from(&state.suggestions[0].value);
                *state.selected_index = None;
                let _ = state.tx.send(state.input_buffer.clone());
                *state.needs_render = true;
            }
            Ok(None)
        }
        KeyCode::Backspace => {
            state.input_buffer.pop();
            *state.selected_index = None;
            let _ = state.tx.send(state.input_buffer.clone());
            *state.needs_render = true;
            Ok(None)
        }
        KeyCode::Char(c) => {
            state.input_buffer.push(c);
            *state.selected_index = None;
            let _ = state.tx.send(state.input_buffer.clone());
            *state.needs_render = true;
            Ok(None)
        }
        _ => Ok(None),
    }
}

/// Interactively prompts the user for the competition ID or file path with real-time,
/// debounced WCA API autocomplete and local file suggestions.
pub fn prompt_competition_source() -> Result<String, InteractiveError> {
    let _raw_guard = RawModeGuard::enter()?;
    let mut out = stdout();

    let (tx, rx) = mpsc::channel::<String>();
    let shared = Arc::new(Mutex::new(SharedSearchState {
        is_loading: false,
        api_cache: std::collections::HashMap::new(),
        api_results: Vec::new(),
        api_version: 0,
    }));

    spawn_search_worker(rx, Arc::clone(&shared));

    let mut input_buffer = String::new();
    let mut selected_index: Option<usize> = None;
    let mut previous_rendered_lines: usize = 0;
    let mut needs_render = true;
    let mut last_api_version = 0;
    let mut last_loading = false;

    loop {
        // Check if background worker updated results or loading state
        if let Ok(state) = shared.lock()
            && (state.api_version != last_api_version || state.is_loading != last_loading)
        {
            needs_render = true;
            last_api_version = state.api_version;
            last_loading = state.is_loading;
        }

        // Collect current suggestions
        let local_suggestions = get_local_json_suggestions(&input_buffer);
        let (api_suggestions, is_loading) = {
            let state = shared.lock().unwrap();
            (state.api_results.clone(), state.is_loading)
        };

        let mut suggestions = local_suggestions;
        let mut api_suggestions = api_suggestions;
        api_suggestions.retain(|api| !suggestions.iter().any(|s| s.value == api.value));
        suggestions.extend(api_suggestions);

        // Clamp selected index
        if let Some(idx) = selected_index {
            if suggestions.is_empty() {
                selected_index = None;
            } else if idx >= suggestions.len() {
                selected_index = Some(suggestions.len() - 1);
            }
        }

        if needs_render {
            previous_rendered_lines = render_search_widget(
                &mut out,
                &input_buffer,
                &suggestions,
                selected_index,
                is_loading,
                previous_rendered_lines,
            )?;
            needs_render = false;
        }

        // Poll for event with 50ms timeout for fluid non-blocking UI
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(
                key @ KeyEvent {
                    kind: event::KeyEventKind::Press,
                    ..
                },
            ) = event::read()?
        {
            let mut state = SearchInputState {
                input_buffer: &mut input_buffer,
                selected_index: &mut selected_index,
                needs_render: &mut needs_render,
                suggestions: &suggestions,
                tx: &tx,
            };
            if let Some(chosen) =
                handle_search_key(&mut state, key, &mut out, previous_rendered_lines)?
            {
                return Ok(chosen);
            }
        }
    }
}
