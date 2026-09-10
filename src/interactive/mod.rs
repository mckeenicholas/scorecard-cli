pub mod prompts;
pub mod suggest;
pub mod types;
pub mod widget;

pub use types::InteractiveError;

use prompts::{
    prompt_competition_and_load, prompt_cover_sheets_selection, prompt_extra_options_selection,
    prompt_page_format, prompt_paper_size, prompt_rounds_selection, prompt_split_selection,
};

use crate::options::{Cli, ResolvedOptions};
use crate::wcif::Competition;

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
mod tests;
