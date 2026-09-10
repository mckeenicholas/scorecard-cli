use std::collections::HashSet;

use inquire::{MultiSelect, Select};

use super::types::{CoverSheetChoice, ExtraFlags, ExtraOption, InteractiveError, RoundChoice};
use super::widget;
use crate::options::{CoverSheetBy, ResolvedOptions, SplitBy};
use crate::pdf::{PageFormat, PaperSize};
use crate::scorecard::{RoundId, ScorecardPlanner, WcaEvent, events};
use crate::wcif::{Competition, WcifLoader};

pub fn list_select_prompt(option: &str) -> String {
    format!("{option}  (use ↑/↓ arrows, Enter to select):")
}

pub fn toggle_select_prompt(option: &str) -> String {
    format!("{option} (Space to toggle checkboxes, Enter to confirm):")
}

pub fn prompt_competition_and_load() -> Result<(String, Competition), InteractiveError> {
    loop {
        let source = widget::prompt_competition_source()?;
        let trimmed = source.trim();
        if trimmed.is_empty() {
            println!("Please enter a competition ID or path.\n");
            continue;
        }
        println!("Loading WCIF from: {trimmed}...");
        match WcifLoader::load(trimmed) {
            Ok(c) => {
                println!("Loaded competition: {} ({})\n", c.name, c.id);
                return Ok((trimmed.to_owned(), c));
            }
            Err(e) => {
                println!("Could not load competition '{trimmed}': {e}. Please try again.\n");
            }
        }
    }
}

pub fn prompt_rounds_selection(comp: &Competition) -> Result<Vec<String>, InteractiveError> {
    let (default_targets, _) = ScorecardPlanner::resolve_all_targets(comp);
    let default_round_ids: HashSet<RoundId> =
        default_targets.into_iter().map(|t| t.round_id).collect();

    let round_choices: Vec<RoundChoice> = comp
        .events
        .iter()
        .filter(|e| e.id != "333fm")
        .flat_map(|event| {
            let event_name = events::event_name_by_id(&event.id).unwrap_or(&event.id);
            let opt_wca_event = WcaEvent::from_id(&event.id);
            event
                .rounds
                .iter()
                .enumerate()
                .filter_map(move |(round_idx, round)| {
                    let wca_event = opt_wca_event?;
                    let round_num = u32::try_from(round_idx + 1).ok()?;
                    let round_id = RoundId::new(wca_event, round_num);
                    Some(RoundChoice {
                        round_id,
                        display: format!("{event_name} - Round {round_num} ({})", round.id),
                    })
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

    Ok(selected_rounds
        .into_iter()
        .map(|r| r.round_id.to_string())
        .collect())
}

pub fn prompt_paper_size(default_paper: PaperSize) -> Result<PaperSize, InteractiveError> {
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

pub fn prompt_page_format(default_format: PageFormat) -> Result<PageFormat, InteractiveError> {
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

pub fn prompt_cover_sheets_selection(
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

pub fn prompt_split_selection() -> Result<Option<Vec<SplitBy>>, InteractiveError> {
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

pub fn prompt_extra_options_selection(
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
