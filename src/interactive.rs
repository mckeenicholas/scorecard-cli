use crate::options::{Cli, CoverSheetBy, ResolvedOptions, SplitBy};
use crate::pdf::{PageFormat, PaperSize};
use crate::scorecard::{ScorecardPlanner, events::event_name_by_id};
use crate::wcif::{Competition, WcifLoader, expand_tilde};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{self, Stylize},
    terminal::{self, ClearType},
};
use inquire::{MultiSelect, Select};
use std::collections::HashSet;
use std::io::{Write, stdout};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

#[derive(Debug, Clone)]
struct Suggestion {
    value: String,
    display: String,
}

#[derive(Debug, Clone)]
struct RoundChoice {
    round_id: String,
    display: String,
}

impl std::fmt::Display for RoundChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoverSheetChoice {
    Round,
    Group,
    Stage,
}

impl std::fmt::Display for CoverSheetChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoverSheetChoice::Round => write!(f, "By Round (one for the entire round)"),
            CoverSheetChoice::Group => write!(f, "By Group (one per group across all stages)"),
            CoverSheetChoice::Stage => write!(f, "By Stage (one per group on each stage)"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtraOption {
    StartGroupOnNewPage,
    PrintStations,
    LocalNamesFirst,
    PrintOneName,
    ScrambleCheckerTopRanked,
    ScrambleCheckerFinalRounds,
    ScrambleCheckerBlank,
}

impl std::fmt::Display for ExtraOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtraOption::StartGroupOnNewPage => {
                write!(
                    f,
                    "Start group on new page (insert blank spaces to align top-left)"
                )
            }
            ExtraOption::PrintStations => write!(f, "Print station numbers"),
            ExtraOption::LocalNamesFirst => write!(f, "Display local names first"),
            ExtraOption::PrintOneName => write!(f, "Only print one name"),
            ExtraOption::ScrambleCheckerTopRanked => {
                write!(f, "Scramble checker for top ranked competitors")
            }
            ExtraOption::ScrambleCheckerFinalRounds => {
                write!(f, "Scramble checker for final rounds")
            }
            ExtraOption::ScrambleCheckerBlank => {
                write!(f, "Scramble checker for blank scorecards")
            }
        }
    }
}

/// Error encountered during the interactive terminal prompt flow.
#[derive(Debug)]
pub enum InteractiveError {
    Io(std::io::Error),
    Prompt(inquire::InquireError),
    Wcif(crate::wcif::WcifLoadError),
    Aborted,
}

impl std::fmt::Display for InteractiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InteractiveError::Io(e) => write!(f, "Interactive I/O error: {e}"),
            InteractiveError::Prompt(e) => write!(f, "Interactive prompt error: {e}"),
            InteractiveError::Wcif(e) => write!(f, "{e}"),
            InteractiveError::Aborted => write!(f, "Interactive flow aborted by user"),
        }
    }
}

impl std::error::Error for InteractiveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            InteractiveError::Io(e) => Some(e),
            InteractiveError::Prompt(e) => Some(e),
            InteractiveError::Wcif(e) => Some(e),
            InteractiveError::Aborted => None,
        }
    }
}

impl From<std::io::Error> for InteractiveError {
    fn from(err: std::io::Error) -> Self {
        InteractiveError::Io(err)
    }
}

impl From<inquire::InquireError> for InteractiveError {
    fn from(err: inquire::InquireError) -> Self {
        InteractiveError::Prompt(err)
    }
}

impl From<crate::wcif::WcifLoadError> for InteractiveError {
    fn from(err: crate::wcif::WcifLoadError) -> Self {
        InteractiveError::Wcif(err)
    }
}

struct RawModeGuard;

impl RawModeGuard {
    fn enter() -> Result<Self, std::io::Error> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(stdout(), cursor::Show);
    }
}

#[derive(serde::Deserialize)]
struct WcaItem {
    id: String,
    name: String,
    country_iso2: Option<String>,
}

fn list_select_prompt(option: &str) -> String {
    format!("{option}  (use ↑/↓ arrows, Enter to select):")
}

fn toggle_select_prompt(option: &str) -> String {
    format!("{option} (Space to toggle checkboxes, Enter to confirm):")
}

/// Scans the local filesystem for .json files and directories.
/// Supports relative paths ('.' and 'tests/'), '~' (home directory), and '/' (root directory).
fn get_local_json_suggestions(query: &str) -> Vec<Suggestion> {
    let trimmed = query.trim();

    let is_path_mode = trimmed.starts_with('~')
        || trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || trimmed.starts_with('.')
        || trimmed.contains('/')
        || trimmed.contains('\\');

    if is_path_mode {
        get_path_suggestions(trimmed)
    } else {
        get_relative_json_suggestions(trimmed)
    }
}

/// Fallback / default search scanning '.' and 'tests/' for JSON files matching the name query.
fn get_relative_json_suggestions(query: &str) -> Vec<Suggestion> {
    let query_lower = query.to_lowercase();
    let mut files: Vec<String> = [".", "tests"]
        .into_iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && let Some(s) = path.to_str()
            {
                let clean = s.trim_start_matches("./").to_string();
                if query_lower.is_empty() || clean.to_lowercase().contains(&query_lower) {
                    return Some(clean);
                }
            }
            None
        })
        .collect();

    files.sort();
    files.dedup();

    files
        .into_iter()
        .map(|f| Suggestion {
            value: f.clone(),
            display: format!("{f} [local file]"),
        })
        .collect()
}

fn parse_path_query(query: &str) -> (std::path::PathBuf, String, &str) {
    if query == "~" {
        (expand_tilde("~"), "~/".to_string(), "")
    } else if let Some(last_sep) = query.rfind(['/', '\\']) {
        let parent_str = &query[..=last_sep];
        let filter = &query[last_sep + 1..];
        let scan_path = expand_tilde(parent_str);
        (scan_path, parent_str.to_string(), filter)
    } else {
        (
            expand_tilde("~"),
            "~/".to_string(),
            query.trim_start_matches('~'),
        )
    }
}

/// Dynamic path-based suggestions supporting '~', '/', subdirectories, and JSON files.
fn get_path_suggestions(query: &str) -> Vec<Suggestion> {
    let (scan_dir, display_prefix, filter) = parse_path_query(query);
    let filter_lower = filter.to_lowercase();

    let (mut file_suggestions, mut dir_suggestions) = std::fs::read_dir(&scan_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name_str = entry.file_name().to_string_lossy().into_owned();

            // Ignore hidden files/folders unless query filter explicitly begins with '.'
            if name_str.starts_with('.') && !filter.starts_with('.') {
                return None;
            }

            if !filter_lower.is_empty() && !name_str.to_lowercase().contains(&filter_lower) {
                return None;
            }

            let path = entry.path();
            if path.is_dir() {
                Some((
                    false,
                    Suggestion {
                        value: format!("{display_prefix}{name_str}/"),
                        display: format!("{display_prefix}{name_str}/ [dir]"),
                    },
                ))
            } else if path.extension().is_some_and(|ext| ext == "json") {
                Some((
                    true,
                    Suggestion {
                        value: format!("{display_prefix}{name_str}"),
                        display: format!("{display_prefix}{name_str} [local file]"),
                    },
                ))
            } else {
                None
            }
        })
        .fold(
            (Vec::new(), Vec::new()),
            |(mut files, mut dirs), (is_file, suggestion)| {
                if is_file {
                    files.push(suggestion);
                } else {
                    dirs.push(suggestion);
                }
                (files, dirs)
            },
        );

    file_suggestions.sort_by(|a, b| a.value.cmp(&b.value));
    dir_suggestions.sort_by(|a, b| a.value.cmp(&b.value));

    // Show JSON files first, then directories
    file_suggestions
        .into_iter()
        .chain(dir_suggestions)
        .take(40)
        .collect()
}

/// Converts deserialized WCA items into formatted interactive suggestions.
fn wca_items_to_suggestions(items: Vec<WcaItem>) -> Vec<Suggestion> {
    let id_width = items
        .iter()
        .take(8)
        .map(|item| item.id.len())
        .max()
        .unwrap_or(20)
        .max(20);

    items
        .into_iter()
        .take(8)
        .map(|item| {
            let display =
                format_wca_suggestion(&item.id, &item.name, item.country_iso2.as_deref(), id_width);
            Suggestion {
                value: item.id,
                display,
            }
        })
        .collect()
}

/// Queries the WCA API for competitions matching the search term.
fn fetch_wca_competitions(query: &str) -> Result<Vec<Suggestion>, ureq::Error> {
    let agent: ureq::Agent = ureq::config::Config::builder()
        .timeout_global(Some(Duration::from_millis(2500)))
        .http_status_as_error(false)
        .build()
        .into();

    let mut resp = agent
        .get("https://www.worldcubeassociation.org/api/v0/competitions")
        .query("q", query)
        .call()?;

    if !resp.status().is_success() {
        return Ok(Vec::new());
    }

    let items: Vec<WcaItem> = resp.body_mut().read_json()?;
    Ok(wca_items_to_suggestions(items))
}

/// Formats a WCA competition search suggestion into two aligned columns: ID and Name (with country).
fn format_wca_suggestion(
    id: &str,
    name: &str,
    country_iso2: Option<&str>,
    id_width: usize,
) -> String {
    use std::fmt::Write as FmtWrite;
    let mut s = String::with_capacity(id_width.max(id.len()) + name.len() + 10);
    let _ = write!(s, "{id:<id_width$}  {name}");
    if let Some(c) = country_iso2 {
        let _ = write!(s, " ({c})");
    }
    s
}

struct SharedSearchState {
    is_loading: bool,
    api_cache: std::collections::HashMap<String, Vec<Suggestion>>,
    api_results: Vec<Suggestion>,
    api_version: usize,
}

/// Helper function to clear previous rendered lines from row 0 downwards and return to row 0.
fn clear_widget_lines<W: Write>(out: &mut W, lines_count: usize) -> Result<(), std::io::Error> {
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

fn spawn_search_worker(rx: mpsc::Receiver<String>, shared: Arc<Mutex<SharedSearchState>>) {
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

fn render_search_widget<W: Write>(
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

struct SearchInputState<'a> {
    input_buffer: &'a mut String,
    selected_index: &'a mut Option<usize>,
    needs_render: &'a mut bool,
    suggestions: &'a [Suggestion],
    tx: &'a mpsc::Sender<String>,
}

fn handle_search_key(
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
fn prompt_competition_source() -> Result<String, InteractiveError> {
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

fn prompt_competition_and_load() -> Result<(String, Competition), InteractiveError> {
    loop {
        let source = prompt_competition_source()?;
        let trimmed = source.trim();
        if trimmed.is_empty() {
            println!("Please enter a competition ID or path.\n");
            continue;
        }
        println!("Loading WCIF from: {trimmed}...");
        match WcifLoader::load(trimmed) {
            Ok(c) => {
                println!("Loaded competition: {} ({})\n", c.name, c.id);
                return Ok((trimmed.to_string(), c));
            }
            Err(e) => {
                println!("Could not load competition '{trimmed}': {e}. Please try again.\n");
            }
        }
    }
}

fn prompt_rounds_selection(comp: &Competition) -> Result<Vec<String>, InteractiveError> {
    let (default_targets, _) = ScorecardPlanner::resolve_all_targets(comp);
    let default_round_ids: HashSet<String> =
        default_targets.into_iter().map(|t| t.round_id).collect();

    let round_choices: Vec<RoundChoice> = comp
        .events
        .iter()
        .filter(|e| e.id != "333fm")
        .flat_map(|event| {
            let event_name = event_name_by_id(&event.id).unwrap_or(&event.id);
            event
                .rounds
                .iter()
                .enumerate()
                .map(move |(round_idx, round)| {
                    let round_num = round_idx + 1;
                    RoundChoice {
                        round_id: round.id.clone(),
                        display: format!("{event_name} - Round {round_num} ({})", round.id),
                    }
                })
        })
        .collect();

    let default_round_indices: Vec<usize> = round_choices
        .iter()
        .enumerate()
        .filter(|(_, choice)| default_round_ids.contains(&choice.round_id))
        .map(|(idx, _)| idx)
        .collect();

    let selected_rounds =
        MultiSelect::new(&toggle_select_prompt("Events and rounds"), round_choices)
            .with_default(&default_round_indices)
            .prompt()?;

    Ok(selected_rounds.into_iter().map(|r| r.round_id).collect())
}

fn prompt_paper_size(default_paper: PaperSize) -> Result<PaperSize, InteractiveError> {
    let paper_options = vec![PaperSize::A4, PaperSize::Letter, PaperSize::A6];
    let default_paper_idx = paper_options
        .iter()
        .position(|p| *p == default_paper)
        .unwrap_or(0);

    let paper = Select::new(&list_select_prompt("Paper Size"), paper_options)
        .with_starting_cursor(default_paper_idx)
        .prompt()?;

    Ok(paper)
}

fn prompt_page_format(default_format: PageFormat) -> Result<PageFormat, InteractiveError> {
    let format_options = vec![PageFormat::Group, PageFormat::Stacked];
    let default_format_idx = format_options
        .iter()
        .position(|f| *f == default_format)
        .unwrap_or(0);

    let format = Select::new(
        "Page layout format (use ↑/↓ arrows, Enter to select):",
        format_options,
    )
    .with_starting_cursor(default_format_idx)
    .prompt()?;

    Ok(format)
}

fn prompt_cover_sheets_selection(
    default_opts: &ResolvedOptions,
) -> Result<Option<Vec<CoverSheetBy>>, InteractiveError> {
    let cover_choices = vec![
        CoverSheetChoice::Round,
        CoverSheetChoice::Group,
        CoverSheetChoice::Stage,
    ];

    let default_cover_indices = if default_opts.cover_sheets {
        let indices: Vec<usize> = cover_choices
            .iter()
            .enumerate()
            .filter(|(_, choice)| match choice {
                CoverSheetChoice::Round => {
                    default_opts.cover_sheets_by.contains(&CoverSheetBy::Round)
                }
                CoverSheetChoice::Group => {
                    default_opts.cover_sheets_by.contains(&CoverSheetBy::Group)
                }
                CoverSheetChoice::Stage => {
                    default_opts.cover_sheets_by.contains(&CoverSheetBy::Stage)
                }
            })
            .map(|(idx, _)| idx)
            .collect();
        if indices.is_empty() {
            vec![2] // Default to Stage if cover_sheets was enabled without criteria
        } else {
            indices
        }
    } else {
        Vec::new()
    };

    let selected_cover = MultiSelect::new(
        "Include cover sheets (Space to toggle checkboxes, leave empty for none):",
        cover_choices,
    )
    .with_default(&default_cover_indices)
    .prompt()?;

    if selected_cover.is_empty() {
        Ok(None)
    } else {
        Ok(Some(
            selected_cover
                .into_iter()
                .map(|c| match c {
                    CoverSheetChoice::Round => CoverSheetBy::Round,
                    CoverSheetChoice::Group => CoverSheetBy::Group,
                    CoverSheetChoice::Stage => CoverSheetBy::Stage,
                })
                .collect(),
        ))
    }
}

fn prompt_split_selection() -> Result<Option<Vec<SplitBy>>, InteractiveError> {
    let split_selected = MultiSelect::new(
        "Split scorecards into separate PDFs by (Space to toggle, Enter to confirm):",
        vec![SplitBy::Event, SplitBy::Group, SplitBy::Stage],
    )
    .with_help_message("Leave empty to generate a single combined PDF")
    .prompt()?;

    if split_selected.is_empty() {
        Ok(None)
    } else {
        Ok(Some(split_selected))
    }
}

struct ExtraFlags {
    start_group_on_new_page: bool,
    print_stations: bool,
    local_names_first: bool,
    print_one_name: bool,
    scramble_checker_top_ranked: bool,
    scramble_checker_final_rounds: bool,
    scramble_checker_blank: bool,
}

fn prompt_extra_options_selection(
    default_opts: &ResolvedOptions,
    format: PageFormat,
) -> Result<ExtraFlags, InteractiveError> {
    let mut extra_options_list = Vec::new();
    if format == PageFormat::Group {
        extra_options_list.push(ExtraOption::StartGroupOnNewPage);
    }
    extra_options_list.extend([
        ExtraOption::PrintStations,
        ExtraOption::LocalNamesFirst,
        ExtraOption::PrintOneName,
        ExtraOption::ScrambleCheckerTopRanked,
        ExtraOption::ScrambleCheckerFinalRounds,
        ExtraOption::ScrambleCheckerBlank,
    ]);

    let default_extra_indices: Vec<usize> = extra_options_list
        .iter()
        .enumerate()
        .filter(|(_, opt)| match opt {
            ExtraOption::StartGroupOnNewPage => default_opts.start_group_on_new_page,
            ExtraOption::PrintStations => default_opts.print_stations,
            ExtraOption::LocalNamesFirst => default_opts.local_names_first,
            ExtraOption::PrintOneName => default_opts.print_one_name,
            ExtraOption::ScrambleCheckerTopRanked => default_opts.scramble_checker_top_ranked,
            ExtraOption::ScrambleCheckerFinalRounds => default_opts.scramble_checker_final_rounds,
            ExtraOption::ScrambleCheckerBlank => default_opts.scramble_checker_blank,
        })
        .map(|(idx, _)| idx)
        .collect();

    let extra_options = MultiSelect::new(
        &toggle_select_prompt("Additional Options"),
        extra_options_list,
    )
    .with_default(&default_extra_indices)
    .prompt()?;

    Ok(ExtraFlags {
        start_group_on_new_page: extra_options.contains(&ExtraOption::StartGroupOnNewPage),
        print_stations: extra_options.contains(&ExtraOption::PrintStations),
        local_names_first: extra_options.contains(&ExtraOption::LocalNamesFirst),
        print_one_name: extra_options.contains(&ExtraOption::PrintOneName),
        scramble_checker_top_ranked: extra_options.contains(&ExtraOption::ScrambleCheckerTopRanked),
        scramble_checker_final_rounds: extra_options
            .contains(&ExtraOption::ScrambleCheckerFinalRounds),
        scramble_checker_blank: extra_options.contains(&ExtraOption::ScrambleCheckerBlank),
    })
}

/// Runs the interactive terminal UI flow, prompting the user with pre-selected defaults from WCIF.
pub fn prompt_interactive_flow() -> Result<(Cli, Competition), InteractiveError> {
    let (comp_source, comp) = prompt_competition_and_load()?;
    let default_opts =
        ResolvedOptions::resolve(&Cli::default(), comp.get_groupifier_config().as_ref());

    let events = prompt_rounds_selection(&comp)?;
    let paper = prompt_paper_size(default_opts.paper)?;
    let format = prompt_page_format(default_opts.format)?;
    let cover_sheets = prompt_cover_sheets_selection(&default_opts)?;
    let split = prompt_split_selection()?;
    let extras = prompt_extra_options_selection(&default_opts, format)?;

    let cli = Cli {
        comp_source: Some(comp_source),
        events,
        paper: Some(paper),
        format: Some(format),
        split,
        cover_sheets,
        local_names_first: Some(extras.local_names_first),
        print_one_name: Some(extras.print_one_name),
        print_stations: Some(extras.print_stations),
        scramble_checker_top_ranked: Some(extras.scramble_checker_top_ranked),
        scramble_checker_final_rounds: Some(extras.scramble_checker_final_rounds),
        scramble_checker_blank: Some(extras.scramble_checker_blank),
        start_group_on_new_page: Some(extras.start_group_on_new_page),
        font: None,
    };

    Ok((cli, comp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_json_suggestions() {
        let suggestions = get_local_json_suggestions("test_wcif");
        assert!(
            suggestions
                .iter()
                .any(|s| s.value.contains("tests/test_wcif.json")
                    || s.value.contains("test_wcif.json")),
            "Expected test_wcif.json to be found in local json suggestions"
        );

        let empty_query = get_local_json_suggestions("");
        assert!(
            !empty_query.is_empty(),
            "Expected empty query to return all local json files"
        );
    }

    #[test]
    fn test_tilde_and_root_path_suggestions() {
        let root_suggestions = get_local_json_suggestions("/");
        assert!(
            !root_suggestions.is_empty(),
            "Expected root path suggestions to return directories"
        );
        assert!(
            root_suggestions.iter().any(|s| s.value.starts_with('/')),
            "Root suggestions should begin with /"
        );

        let tilde_suggestions = get_local_json_suggestions("~");
        assert!(
            !tilde_suggestions.is_empty(),
            "Expected home path suggestions to return entries"
        );
        assert!(
            tilde_suggestions.iter().any(|s| s.value.starts_with("~/")),
            "Tilde suggestions should begin with ~/"
        );

        let tilde_slash = get_local_json_suggestions("~/");
        assert!(
            !tilde_slash.is_empty(),
            "Expected ~/ suggestions to return entries"
        );
    }

    #[test]
    fn test_cover_sheet_choice_display() {
        assert_eq!(
            CoverSheetChoice::Stage.to_string(),
            "By Stage (one per group on each stage)"
        );
        assert_eq!(
            CoverSheetChoice::Group.to_string(),
            "By Group (one per group across all stages)"
        );
        assert_eq!(
            CoverSheetChoice::Round.to_string(),
            "By Round (one for the entire round)"
        );
    }

    #[test]
    fn test_extra_option_display() {
        assert!(
            ExtraOption::StartGroupOnNewPage
                .to_string()
                .contains("Start group on new page")
        );
        assert_eq!(
            ExtraOption::PrintStations.to_string(),
            "Print station numbers"
        );
        assert_eq!(ExtraOption::PrintOneName.to_string(), "Only print one name");
    }

    #[test]
    fn test_format_wca_suggestion_alignment() {
        let s1 = format_wca_suggestion("NAC2026", "North American Championship", Some("US"), 22);
        let s2 = format_wca_suggestion("AjaxAutumnAM2026", "Ajax Autumn AM 2026", Some("CA"), 22);
        assert_eq!(
            s1,
            "NAC2026                 North American Championship (US)"
        );
        assert_eq!(s2, "AjaxAutumnAM2026        Ajax Autumn AM 2026 (CA)");
        // Column 2 starts at index 24 (22 chars for ID + 2 spaces)
        assert_eq!(&s1[..24], "NAC2026                 ");
        assert_eq!(&s2[..24], "AjaxAutumnAM2026        ");
    }
}
