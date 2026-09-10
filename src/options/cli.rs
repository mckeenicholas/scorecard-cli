use std::path::PathBuf;

use clap::Parser;

use super::types::{CoverSheetBy, SplitBy};
use crate::pdf::{PageFormat, PaperSize};

/// Fast WCA Cubing Competition Scorecard Generator in Rust
#[derive(Parser, Debug, Clone, Default)]
#[command(
    name = "scorecard-gen",
    about = "Fast cubing competition scorecard generator in Rust",
    version
)]
pub struct Cli {
    /// Competition name/ID or WCIF file path
    #[arg(value_name = "COMPETITION")]
    pub comp_source: Option<String>,

    /// Events and rounds (e.g. 333, 333-2, 333-r2)
    #[arg(value_name = "EVENTS")]
    pub events: Vec<String>,

    /// Set paper size (a4, letter, a6)
    #[arg(
        short = 'p',
        long,
        visible_alias = "scorecard-paper-size",
        value_enum,
        value_name = "PAPER"
    )]
    pub paper: Option<PaperSize>,

    /// Set page layout format (group, stacked)
    #[arg(short = 'f', long, value_enum, value_name = "FORMAT")]
    pub format: Option<PageFormat>,

    /// Split scorecards into separate PDFs by event, group, stage, or a combination
    #[arg(
        short = 's',
        long = "split",
        visible_alias = "shard",
        value_enum,
        value_name = "CRITERIA",
        value_delimiter = ',',
        num_args = 1..
    )]
    pub split: Option<Vec<SplitBy>>,

    /// Include cover sheets. Optionally specify criteria: r (round), g (group), s (stage). Defaults to stage (s).
    #[arg(
        short = 'c',
        long = "cover-sheets",
        visible_alias = "print-scorecards-cover-sheets",
        value_enum,
        value_delimiter = ',',
        num_args = 0..,
        default_missing_value = "stage",
        value_name = "CRITERIA"
    )]
    pub cover_sheets: Option<Vec<CoverSheetBy>>,

    /// Display local names first
    #[arg(
        short = 'l',
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub local_names_first: Option<bool>,

    /// Only print one name
    #[arg(
        short = 'n',
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub print_one_name: Option<bool>,

    /// Print station numbers
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub print_stations: Option<bool>,

    /// Print scramble checker for top ranked competitors
    #[arg(
        short = 't',
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub scramble_checker_top_ranked: Option<bool>,

    /// Print scramble checker for final rounds
    #[arg(
        short = 'r',
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub scramble_checker_final_rounds: Option<bool>,

    /// Print scramble checker for blank scorecards
    #[arg(
        short = 'b',
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub scramble_checker_blank: Option<bool>,

    /// Start each group on a new page (inserts blank spaces so the first scorecard/cover sheet of a group is at top-left)
    #[arg(
        long = "start-group-on-new-page",
        visible_alias = "group-new-page",
        visible_alias = "new-page-per-group",
        visible_alias = "start-groups-on-new-page",
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub start_group_on_new_page: Option<bool>,

    /// Path to a custom TTF/OTF/TTC font file for rendering local competitor names (auto-detected if omitted)
    #[arg(long, value_name = "PATH")]
    pub font: Option<PathBuf>,
}
