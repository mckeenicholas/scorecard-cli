use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use ureq::config::Config;
use ureq::{Agent, Error as UreqError};

use super::types::{Suggestion, WcaItem};
use crate::wcif;

/// Scans the local filesystem for .json files and directories.
/// Supports relative paths ('.' and 'tests/'), '~' (home directory), and '/' (root directory).
pub fn get_local_json_suggestions(query: &str) -> Vec<Suggestion> {
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
pub fn get_relative_json_suggestions(query: &str) -> Vec<Suggestion> {
    let query_lower = query.to_lowercase();
    let mut files: Vec<String> = [".", "tests"]
        .into_iter()
        .filter_map(|dir| fs::read_dir(dir).ok())
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && let Some(s) = path.to_str()
            {
                let clean = s.trim_start_matches("./").to_owned();
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

pub fn parse_path_query(query: &str) -> (PathBuf, String, &str) {
    if query == "~" {
        (wcif::expand_tilde("~"), "~/".to_owned(), "")
    } else if let Some(last_sep) = query.rfind(['/', '\\']) {
        let parent_str = &query[..=last_sep];
        let filter = &query[last_sep + 1..];
        let scan_path = wcif::expand_tilde(parent_str);
        (scan_path, parent_str.to_owned(), filter)
    } else {
        (
            wcif::expand_tilde("~"),
            "~/".to_owned(),
            query.trim_start_matches('~'),
        )
    }
}

/// Dynamic path-based suggestions supporting '~', '/', subdirectories, and JSON files.
pub fn get_path_suggestions(query: &str) -> Vec<Suggestion> {
    let (scan_dir, display_prefix, filter) = parse_path_query(query);
    let filter_lower = filter.to_lowercase();

    let (mut file_suggestions, mut dir_suggestions) = fs::read_dir(&scan_dir)
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
pub fn wca_items_to_suggestions(items: Vec<WcaItem>) -> Vec<Suggestion> {
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
pub fn fetch_wca_competitions(query: &str) -> Result<Vec<Suggestion>, UreqError> {
    let agent: Agent = Config::builder()
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
pub fn format_wca_suggestion(
    id: &str,
    name: &str,
    country_iso2: Option<&str>,
    id_width: usize,
) -> String {
    let mut s = String::with_capacity(id_width.max(id.len()) + name.len() + 10);
    let _ = write!(s, "{id:<id_width$}  {name}");
    if let Some(c) = country_iso2 {
        let _ = write!(s, " ({c})");
    }
    s
}
