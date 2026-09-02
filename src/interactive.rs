use crate::options::{Cli, CoverSheetBy, ResolvedOptions, ShardBy};
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
use std::error::Error;
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
    Stage,
    Group,
    Round,
}

impl std::fmt::Display for CoverSheetChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoverSheetChoice::Stage => write!(f, "By Stage (one per group on each stage)"),
            CoverSheetChoice::Group => write!(f, "By Group (one per group across all stages)"),
            CoverSheetChoice::Round => write!(f, "By Round (one for the entire round)"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtraOption {
    PrintStations,
    AsciiOnly,
    LocalNamesFirst,
    PrintOneName,
    ScrambleCheckerTopRanked,
    ScrambleCheckerFinalRounds,
    ScrambleCheckerBlank,
}

impl std::fmt::Display for ExtraOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtraOption::PrintStations => write!(f, "Print station numbers"),
            ExtraOption::AsciiOnly => write!(f, "ASCII only (no unicode characters)"),
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

struct RawModeGuard;

impl RawModeGuard {
    fn enter() -> Result<Self, Box<dyn Error>> {
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
    let mut files = Vec::new();

    let mut scan_dir = |dir: &str| {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    if let Some(s) = path.to_str() {
                        let clean = s.trim_start_matches("./").to_string();
                        if query_lower.is_empty() || clean.to_lowercase().contains(&query_lower) {
                            files.push(clean);
                        }
                    }
                }
            }
        }
    };

    scan_dir(".");
    scan_dir("tests");
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

/// Dynamic path-based suggestions supporting '~', '/', subdirectories, and JSON files.
fn get_path_suggestions(query: &str) -> Vec<Suggestion> {
    let (scan_dir, display_prefix, filter) = if query == "~" {
        (expand_tilde("~"), "~/".to_string(), "")
    } else if let Some(last_sep) = query.rfind(|c| c == '/' || c == '\\') {
        let parent_str = &query[..=last_sep];
        let filter = &query[last_sep + 1..];
        let scan_path = expand_tilde(parent_str);
        (scan_path, parent_str.to_string(), filter)
    } else {
        (expand_tilde("~"), "~/".to_string(), query.trim_start_matches('~'))
    };

    let filter_lower = filter.to_lowercase();
    let mut file_suggestions = Vec::new();
    let mut dir_suggestions = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&scan_dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();

            // Ignore hidden files/folders unless query filter explicitly begins with '.'
            if name_str.starts_with('.') && !filter.starts_with('.') {
                continue;
            }

            if !filter_lower.is_empty() && !name_str.to_lowercase().contains(&filter_lower) {
                continue;
            }

            let path = entry.path();
            if path.is_dir() {
                let val = format!("{}{}/", display_prefix, name_str);
                let disp = format!("{}{}/ [dir]", display_prefix, name_str);
                dir_suggestions.push(Suggestion {
                    value: val,
                    display: disp,
                });
            } else if path.extension().is_some_and(|ext| ext == "json") {
                let val = format!("{}{}", display_prefix, name_str);
                let disp = format!("{}{} [local file]", display_prefix, name_str);
                file_suggestions.push(Suggestion {
                    value: val,
                    display: disp,
                });
            }
        }
    }

    file_suggestions.sort_by(|a, b| a.value.cmp(&b.value));
    dir_suggestions.sort_by(|a, b| a.value.cmp(&b.value));

    // Show JSON files first, then directories
    let mut results = file_suggestions;
    results.extend(dir_suggestions);
    results.truncate(40);
    results
}

/// Queries the WCA API for competitions matching the search term.
fn fetch_wca_competitions(query: &str) -> Result<Vec<Suggestion>, Box<dyn Error + Send + Sync>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(2500))
        .build()?;

    let encoded_query: String = query
        .chars()
        .flat_map(|c| match c {
            ' ' => vec!['%', '2', '0'],
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => vec![c],
            _ => format!("%{:02X}", c as u32).chars().collect(),
        })
        .collect();
    let url = format!(
        "https://www.worldcubeassociation.org/api/v0/competitions?q={}",
        encoded_query
    );

    let resp = client.get(&url).send()?;

    if !resp.status().is_success() {
        return Ok(Vec::new());
    }

    #[derive(serde::Deserialize)]
    struct WcaItem {
        id: String,
        name: String,
        country_iso2: Option<String>,
    }

    let items: Vec<WcaItem> = resp.json()?;
    let suggestions = items
        .into_iter()
        .take(8)
        .map(|item| {
            let country = item
                .country_iso2
                .map(|c| format!(" ({c})"))
                .unwrap_or_default();
            Suggestion {
                value: item.id.clone(),
                display: format!("{} - {}{}", item.id, item.name, country),
            }
        })
        .collect();

    Ok(suggestions)
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
        queue!(out, cursor::MoveUp((lines_count - 1) as u16))?;
    }
    queue!(out, cursor::MoveToColumn(0))?;
    Ok(())
}

/// Interactively prompts the user for the competition ID or file path with real-time,
/// debounced WCA API autocomplete and local file suggestions.
fn prompt_competition_source() -> Result<String, Box<dyn Error>> {
    let _raw_guard = RawModeGuard::enter()?;
    let mut out = stdout();

    let (tx, rx) = mpsc::channel::<String>();
    let shared = Arc::new(Mutex::new(SharedSearchState {
        is_loading: false,
        api_cache: std::collections::HashMap::new(),
        api_results: Vec::new(),
        api_version: 0,
    }));

    let shared_clone = Arc::clone(&shared);
    let _worker_handle = std::thread::spawn(move || {
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
                if let Ok(mut state) = shared_clone.lock() {
                    state.api_results.clear();
                    state.is_loading = false;
                    state.api_version += 1;
                }
                continue;
            }

            // Check cache
            let already_cached = {
                if let Ok(mut state) = shared_clone.lock() {
                    if let Some(cached) = state.api_cache.get(&trimmed) {
                        state.api_results = cached.clone();
                        state.is_loading = false;
                        state.api_version += 1;
                        true
                    } else {
                        state.is_loading = true;
                        state.api_version += 1;
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

            if let Ok(mut state) = shared_clone.lock() {
                state.api_cache.insert(trimmed, results.clone());
                state.api_results = results;
                state.is_loading = false;
                state.api_version += 1;
            }
        }
    });

    let mut input_buffer = String::new();
    let mut selected_index: Option<usize> = None;
    let mut previous_rendered_lines: usize = 0;
    let mut needs_render = true;
    let mut last_api_version = 0;
    let mut last_loading = false;

    loop {
        // Check if background worker updated results or loading state
        {
            if let Ok(state) = shared.lock() {
                if state.api_version != last_api_version || state.is_loading != last_loading {
                    needs_render = true;
                    last_api_version = state.api_version;
                    last_loading = state.is_loading;
                }
            }
        }

        // Collect current suggestions
        let local_suggestions = get_local_json_suggestions(&input_buffer);
        let (api_suggestions, is_loading) = {
            let state = shared.lock().unwrap();
            (state.api_results.clone(), state.is_loading)
        };

        let mut suggestions = local_suggestions;
        // Append API suggestions that aren't duplicates
        for api_item in api_suggestions {
            if !suggestions.iter().any(|s| s.value == api_item.value) {
                suggestions.push(api_item);
            }
        }

        // Clamp selected index
        if let Some(idx) = selected_index {
            if suggestions.is_empty() {
                selected_index = None;
            } else if idx >= suggestions.len() {
                selected_index = Some(suggestions.len() - 1);
            }
        }

        if needs_render {
            // Clear previous rendered lines from top down
            clear_widget_lines(&mut out, previous_rendered_lines)?;

            let mut lines_rendered = 0;

            // Render prompt line
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
                style::Print(&input_buffer),
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

            for i in start_idx..end_idx {
                let item = &suggestions[i];
                let is_selected = selected_index == Some(i);
                if is_selected {
                    queue!(
                        out,
                        style::SetForegroundColor(style::Color::Cyan),
                        style::Print(format!("  > {}\r\n", item.display)),
                        style::ResetColor,
                    )?;
                } else {
                    queue!(out, style::Print(format!("    {}\r\n", item.display)),)?;
                }
                lines_rendered += 1;
            }

            // Help line (no trailing newline so cursor stays on last rendered line)
            queue!(
                out,
                style::SetForegroundColor(style::Color::DarkGrey),
                style::Print("  [↑/↓ to navigate, Tab to complete, Enter to select]"),
                style::ResetColor,
            )?;
            lines_rendered += 1;

            // Position cursor back on the input line at row 0
            let prompt_prefix = "? Competition ID or WCIF file path: ";
            let col = (prompt_prefix.len() + input_buffer.len()) as u16;
            let rows_to_move_up = (lines_rendered - 1) as u16;
            queue!(
                out,
                cursor::MoveUp(rows_to_move_up),
                cursor::MoveToColumn(col),
                cursor::Show,
            )?;
            out.flush()?;
            previous_rendered_lines = lines_rendered;
            needs_render = false;
        }

        // Poll for event with 50ms timeout for fluid non-blocking UI
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(KeyEvent {
                code,
                modifiers,
                kind: event::KeyEventKind::Press,
                ..
            }) = event::read()?
            {
                match code {
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        return Err("Aborted by user".into());
                    }
                    KeyCode::Esc => {
                        return Err("Aborted by user".into());
                    }
                    KeyCode::Enter => {
                        let chosen = if let Some(idx) = selected_index {
                            suggestions[idx].value.clone()
                        } else {
                            input_buffer.trim().to_string()
                        };

                        // If a directory was selected, navigate into it rather than submitting
                        if chosen.ends_with('/') || chosen.ends_with('\\') {
                            input_buffer = chosen;
                            selected_index = None;
                            let _ = tx.send(input_buffer.clone());
                            needs_render = true;
                            continue;
                        }

                        // Clean up dropdown lines completely
                        clear_widget_lines(&mut out, previous_rendered_lines)?;

                        // Print confirmed line
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
                        return Ok(chosen);
                    }
                    KeyCode::Down => {
                        if !suggestions.is_empty() {
                            selected_index = Some(match selected_index {
                                None => 0,
                                Some(i) => (i + 1).min(suggestions.len() - 1),
                            });
                            needs_render = true;
                        }
                    }
                    KeyCode::Up => {
                        selected_index = match selected_index {
                            None => None,
                            Some(0) => None,
                            Some(i) => Some(i - 1),
                        };
                        needs_render = true;
                    }
                    KeyCode::Tab => {
                        if let Some(idx) = selected_index {
                            input_buffer = suggestions[idx].value.clone();
                            selected_index = None;
                            let _ = tx.send(input_buffer.clone());
                            needs_render = true;
                        } else if !suggestions.is_empty() {
                            input_buffer = suggestions[0].value.clone();
                            selected_index = None;
                            let _ = tx.send(input_buffer.clone());
                            needs_render = true;
                        }
                    }
                    KeyCode::Backspace => {
                        input_buffer.pop();
                        selected_index = None;
                        let _ = tx.send(input_buffer.clone());
                        needs_render = true;
                    }
                    KeyCode::Char(c) => {
                        input_buffer.push(c);
                        selected_index = None;
                        let _ = tx.send(input_buffer.clone());
                        needs_render = true;
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Runs the interactive terminal UI flow, prompting the user with pre-selected defaults from WCIF.
pub fn prompt_interactive_flow() -> Result<(Cli, Competition), Box<dyn Error>> {
    println!("\n=== Scorecard Generator - Interactive Mode ===\n");

    // 1. Prompt for Competition source (ID or local WCIF file path) with debounced autocomplete
    let (comp_source, comp) = loop {
        let source = prompt_competition_source()?;
        let trimmed = source.trim();
        if trimmed.is_empty() {
            println!("Please enter a competition ID or path.\n");
            continue;
        }
        println!("Loading WCIF from: {}...", trimmed);
        match WcifLoader::load(trimmed) {
            Ok(c) => {
                println!("Loaded competition: {} ({})\n", c.name, c.id);
                break (trimmed.to_string(), c);
            }
            Err(e) => {
                println!("Could not load competition '{trimmed}': {e}. Please try again.\n");
            }
        }
    };

    // 2. Resolve default options directly from the WCIF Groupifier config
    let default_opts =
        ResolvedOptions::resolve(&Cli::default(), comp.get_groupifier_config().as_ref());

    // 3. Events and rounds using checkboxes, with ready rounds pre-selected
    let (default_targets, _) = ScorecardPlanner::resolve_targets(&comp, &[]);
    let default_round_ids: HashSet<String> =
        default_targets.into_iter().map(|t| t.round_id).collect();

    let mut round_choices = Vec::new();
    let mut default_round_indices = Vec::new();

    for event in &comp.events {
        if event.id == "333fm" {
            continue;
        }
        let event_name = event_name_by_id(&event.id).unwrap_or(&event.id);
        for (round_idx, round) in event.rounds.iter().enumerate() {
            let round_num = round_idx + 1;
            let display = format!("{event_name} - Round {round_num} ({})", round.id);
            if default_round_ids.contains(&round.id) {
                default_round_indices.push(round_choices.len());
            }
            round_choices.push(RoundChoice {
                round_id: round.id.clone(),
                display,
            });
        }
    }

    let selected_rounds = MultiSelect::new(
        "Events and rounds (Space to toggle checkboxes, Enter to confirm):",
        round_choices,
    )
    .with_default(&default_round_indices)
    .prompt()?;

    let events: Vec<String> = selected_rounds.into_iter().map(|r| r.round_id).collect();

    // 4. Paper size (pre-selected from WCIF)
    let paper_options = vec![PaperSize::A4, PaperSize::Letter, PaperSize::A6];
    let default_paper_idx = paper_options
        .iter()
        .position(|p| *p == default_opts.paper)
        .unwrap_or(0);

    let paper = Select::new(
        "Paper size (use ↑/↓ arrows, Enter to select):",
        paper_options,
    )
    .with_starting_cursor(default_paper_idx)
    .prompt()?;

    // 5. Page layout format (pre-selected from WCIF)
    let format_options = vec![PageFormat::Group, PageFormat::Stacked];
    let default_format_idx = format_options
        .iter()
        .position(|f| *f == default_opts.format)
        .unwrap_or(0);

    let format = Select::new(
        "Page layout format (use ↑/↓ arrows, Enter to select):",
        format_options,
    )
    .with_starting_cursor(default_format_idx)
    .prompt()?;

    // 6. Cover sheets: Checkboxes for By Stage, By Group, By Round.
    // If none are selected, it's equivalent to no cover sheets.
    let cover_choices = vec![
        CoverSheetChoice::Stage,
        CoverSheetChoice::Group,
        CoverSheetChoice::Round,
    ];

    let mut default_cover_indices = Vec::new();
    if default_opts.cover_sheets {
        for (idx, choice) in cover_choices.iter().enumerate() {
            let matches = match choice {
                CoverSheetChoice::Stage => {
                    default_opts.cover_sheets_by.contains(&CoverSheetBy::Stage)
                }
                CoverSheetChoice::Group => {
                    default_opts.cover_sheets_by.contains(&CoverSheetBy::Group)
                }
                CoverSheetChoice::Round => {
                    default_opts.cover_sheets_by.contains(&CoverSheetBy::Round)
                }
            };
            if matches {
                default_cover_indices.push(idx);
            }
        }
        if default_cover_indices.is_empty() {
            default_cover_indices.push(0); // Default to Stage if cover_sheets was enabled without criteria
        }
    }

    let selected_cover = MultiSelect::new(
        "Include cover sheets (Space to toggle checkboxes, leave empty for none):",
        cover_choices,
    )
    .with_default(&default_cover_indices)
    .prompt()?;

    let cover_sheets = if selected_cover.is_empty() {
        None
    } else {
        Some(
            selected_cover
                .into_iter()
                .map(|c| match c {
                    CoverSheetChoice::Stage => CoverSheetBy::Stage,
                    CoverSheetChoice::Group => CoverSheetBy::Group,
                    CoverSheetChoice::Round => CoverSheetBy::Round,
                })
                .collect(),
        )
    };

    // 7. PDF Sharding (Space to toggle, Enter to confirm)
    let shard_selected = MultiSelect::new(
        "Split scorecards into separate PDFs by (Space to toggle, Enter to confirm):",
        vec![ShardBy::Event, ShardBy::Group, ShardBy::Stage],
    )
    .with_help_message("Leave empty to generate a single combined PDF")
    .prompt()?;

    let shard = if shard_selected.is_empty() {
        None
    } else {
        Some(shard_selected)
    };

    // 8. Additional options (checkboxes pre-selected according to WCIF configuration)
    let extra_options_list = vec![
        ExtraOption::PrintStations,
        ExtraOption::AsciiOnly,
        ExtraOption::LocalNamesFirst,
        ExtraOption::PrintOneName,
        ExtraOption::ScrambleCheckerTopRanked,
        ExtraOption::ScrambleCheckerFinalRounds,
        ExtraOption::ScrambleCheckerBlank,
    ];

    let mut default_extra_indices = Vec::new();
    for (idx, opt) in extra_options_list.iter().enumerate() {
        let is_active = match opt {
            ExtraOption::PrintStations => default_opts.print_stations,
            ExtraOption::AsciiOnly => default_opts.ascii,
            ExtraOption::LocalNamesFirst => default_opts.local_names_first,
            ExtraOption::PrintOneName => default_opts.print_one_name,
            ExtraOption::ScrambleCheckerTopRanked => default_opts.scramble_checker_top_ranked,
            ExtraOption::ScrambleCheckerFinalRounds => default_opts.scramble_checker_final_rounds,
            ExtraOption::ScrambleCheckerBlank => default_opts.scramble_checker_blank,
        };
        if is_active {
            default_extra_indices.push(idx);
        }
    }

    let extra_options = MultiSelect::new(
        "Additional options (Space to toggle checkboxes, Enter to confirm):",
        extra_options_list,
    )
    .with_default(&default_extra_indices)
    .prompt()?;

    let ascii = extra_options.contains(&ExtraOption::AsciiOnly);
    let print_stations = Some(extra_options.contains(&ExtraOption::PrintStations));
    let local_names_first = Some(extra_options.contains(&ExtraOption::LocalNamesFirst));
    let print_one_name = Some(extra_options.contains(&ExtraOption::PrintOneName));
    let scramble_checker_top_ranked =
        Some(extra_options.contains(&ExtraOption::ScrambleCheckerTopRanked));
    let scramble_checker_final_rounds =
        Some(extra_options.contains(&ExtraOption::ScrambleCheckerFinalRounds));
    let scramble_checker_blank = Some(extra_options.contains(&ExtraOption::ScrambleCheckerBlank));

    let cli = Cli {
        comp_source: Some(comp_source),
        events,
        paper: Some(paper),
        format: Some(format),
        shard,
        ascii,
        cover_sheets,
        local_names_first,
        print_one_name,
        print_stations,
        scramble_checker_top_ranked,
        scramble_checker_final_rounds,
        scramble_checker_blank,
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
        assert_eq!(
            ExtraOption::PrintStations.to_string(),
            "Print station numbers"
        );
        assert_eq!(
            ExtraOption::AsciiOnly.to_string(),
            "ASCII only (no unicode characters)"
        );
    }
}
