use crate::pdf::{PageFormat, PaperSize};
use crate::wcif::GroupifierCompetitionConfig;
use clap::{Parser, ValueEnum};
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};

/// Criteria for splitting output PDFs into separate files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitBy {
    #[value(name = "event", alias = "events", alias = "e")]
    Event,
    #[value(name = "group", alias = "groups", alias = "g")]
    Group,
    #[value(name = "stage", alias = "stages", alias = "room", alias = "rooms")]
    Stage,
}

impl std::fmt::Display for SplitBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SplitBy::Event => write!(f, "event"),
            SplitBy::Group => write!(f, "group"),
            SplitBy::Stage => write!(f, "stage"),
        }
    }
}

/// Error returned when parsing an invalid split criterion string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseSplitByError(pub String);

impl std::fmt::Display for ParseSplitByError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid split criterion '{}': must be 'event', 'group', or 'stage'",
            self.0
        )
    }
}

impl std::error::Error for ParseSplitByError {}

impl std::str::FromStr for SplitBy {
    type Err = ParseSplitByError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "event" | "events" | "e" => Ok(SplitBy::Event),
            "group" | "groups" | "g" => Ok(SplitBy::Group),
            "stage" | "stages" | "room" | "rooms" | "s" => Ok(SplitBy::Stage),
            other => Err(ParseSplitByError(other.to_string())),
        }
    }
}

/// Criteria for adding cover sheets.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, ValueEnum, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum CoverSheetBy {
    #[value(
        name = "round",
        alias = "rounds",
        alias = "r",
        alias = "event",
        alias = "events",
        alias = "e"
    )]
    Round,
    #[value(name = "group", alias = "groups", alias = "g")]
    Group,
    #[value(
        name = "stage",
        alias = "stages",
        alias = "room",
        alias = "rooms",
        alias = "s"
    )]
    Stage,
    #[value(name = "none", alias = "false", alias = "off")]
    None,
}

impl CoverSheetBy {
    /// Returns the hierarchy tier of the cover sheet criterion (1 is highest tier).
    pub fn tier(self) -> u8 {
        match self {
            CoverSheetBy::Round => 1,
            CoverSheetBy::Group => 2,
            CoverSheetBy::Stage => 3,
            CoverSheetBy::None => 255,
        }
    }
}

impl std::fmt::Display for CoverSheetBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoverSheetBy::Round => write!(f, "round"),
            CoverSheetBy::Group => write!(f, "group"),
            CoverSheetBy::Stage => write!(f, "stage"),
            CoverSheetBy::None => write!(f, "none"),
        }
    }
}

/// Error returned when parsing an invalid cover sheet criterion string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseCoverSheetByError(pub String);

impl std::fmt::Display for ParseCoverSheetByError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid cover sheet criterion '{}': must be 'round' ('r'), 'group' ('g'), or 'stage' ('s')",
            self.0
        )
    }
}

impl std::error::Error for ParseCoverSheetByError {}

impl std::str::FromStr for CoverSheetBy {
    type Err = ParseCoverSheetByError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "round" | "rounds" | "r" | "event" | "events" | "e" => Ok(CoverSheetBy::Round),
            "group" | "groups" | "g" => Ok(CoverSheetBy::Group),
            "stage" | "stages" | "room" | "rooms" | "s" => Ok(CoverSheetBy::Stage),
            "none" | "false" | "off" => Ok(CoverSheetBy::None),
            other => Err(ParseCoverSheetByError(other.to_string())),
        }
    }
}

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
}

/// Error returned when options have incompatible file splitting and cover sheet configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsCompatibilityError {
    /// PDF file splitting is more specific than cover sheet criteria.
    IncompatibleSplit {
        split: SplitBy,
        cover_sheet: CoverSheetBy,
    },
}

impl std::fmt::Display for OptionsCompatibilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionsCompatibilityError::IncompatibleSplit { split, cover_sheet } => {
                match (cover_sheet, split) {
                    (CoverSheetBy::Round, SplitBy::Stage) => write!(
                        f,
                        "Incompatible configuration: Splitting PDF files by 'stage' is more specific than 'round' cover sheet. A round cover sheet cannot be placed in a single stage PDF file."
                    ),
                    (CoverSheetBy::Round, SplitBy::Group) => write!(
                        f,
                        "Incompatible configuration: Splitting PDF files by 'group' is more specific than 'round' cover sheet. A round cover sheet cannot be placed in a single group PDF file."
                    ),
                    (CoverSheetBy::Group, SplitBy::Stage) => write!(
                        f,
                        "Incompatible configuration: Splitting PDF files by 'stage' is more specific than 'group' cover sheet. A group cover sheet covers the entire group across stages."
                    ),
                    _ => write!(
                        f,
                        "Incompatible configuration: Splitting PDF files by '{split}' is more specific than '{cover_sheet}' cover sheet."
                    ),
                }
            }
        }
    }
}

impl std::error::Error for OptionsCompatibilityError {}

/// Fully resolved scorecard generation options after merging CLI flags,
/// WCIF Groupifier config extensions, and default values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOptions {
    pub paper: PaperSize,
    pub format: PageFormat,
    pub cover_sheets: bool,
    pub cover_sheets_by: Vec<CoverSheetBy>,
    pub split: Vec<SplitBy>,
    pub local_names_first: bool, // TODO: wire to renderer
    pub print_one_name: bool,
    pub print_stations: bool,                // TODO: wire to renderer
    pub scramble_checker_top_ranked: bool,   // TODO: wire to renderer
    pub scramble_checker_final_rounds: bool, // TODO: wire to renderer
    pub scramble_checker_blank: bool,        // TODO: wire to renderer
}

impl Default for ResolvedOptions {
    fn default() -> Self {
        Self {
            paper: PaperSize::Letter,
            format: PageFormat::Group,
            cover_sheets: false,
            cover_sheets_by: Vec::new(),
            split: Vec::new(),
            local_names_first: false,
            print_one_name: false,
            print_stations: false,
            scramble_checker_top_ranked: false,
            scramble_checker_final_rounds: false,
            scramble_checker_blank: false,
        }
    }
}

impl ResolvedOptions {
    /// Merges default options, WCIF groupifier settings, and explicit CLI flag overrides.
    pub fn resolve(cli: &Cli, groupifier: Option<&GroupifierCompetitionConfig>) -> Self {
        let mut opts = Self::default();
        if let Some(cfg) = groupifier {
            opts.apply_groupifier_config(cfg);
        }
        opts.apply_cli_overrides(cli);
        opts
    }

    /// Layer 1: Merges configuration from WCIF Groupifier extension.
    fn apply_groupifier_config(&mut self, cfg: &GroupifierCompetitionConfig) {
        if let Some(ref p) = cfg.scorecard_paper_size
            && let Ok(paper) = p.parse::<PaperSize>()
        {
            self.paper = paper;
        }
        if let Some(ref o) = cfg.scorecard_order
            && o.to_lowercase() == "stacked"
        {
            self.format = PageFormat::Stacked;
        }
        if let Some(print_cover) = cfg.print_scorecards_cover_sheets {
            self.cover_sheets = print_cover;
            if print_cover && self.cover_sheets_by.is_empty() {
                self.cover_sheets_by = vec![CoverSheetBy::Stage];
            } else if !print_cover {
                self.cover_sheets_by.clear();
            }
        }
        Self::apply_optional(&mut self.local_names_first, cfg.local_names_first);
        Self::apply_optional(&mut self.print_one_name, cfg.print_one_name);
        Self::apply_optional(&mut self.print_stations, cfg.print_stations);
        Self::apply_optional(
            &mut self.scramble_checker_top_ranked,
            cfg.print_scramble_checker_for_top_ranked_competitors,
        );
        Self::apply_optional(
            &mut self.scramble_checker_final_rounds,
            cfg.print_scramble_checker_for_final_rounds,
        );
        Self::apply_optional(
            &mut self.scramble_checker_blank,
            cfg.print_scramble_checker_for_blank_scorecards,
        );
    }

    /// Layer 2: Applies explicit CLI argument overrides.
    fn apply_cli_overrides(&mut self, cli: &Cli) {
        Self::apply_optional(&mut self.paper, cli.paper);
        Self::apply_optional(&mut self.format, cli.format);

        if let Some(ref cs_list) = cli.cover_sheets {
            if cs_list.contains(&CoverSheetBy::None) {
                self.cover_sheets = false;
                self.cover_sheets_by.clear();
            } else {
                self.cover_sheets = true;
                let mut list = cs_list.clone();
                Self::normalize_cover_sheet_list(&mut list);
                if list.is_empty() {
                    self.cover_sheets_by = vec![CoverSheetBy::Stage];
                } else {
                    self.cover_sheets_by = list;
                }
            }
        } else if self.cover_sheets && self.cover_sheets_by.is_empty() {
            self.cover_sheets_by = vec![CoverSheetBy::Stage];
        }

        if let Some(ref s) = cli.split {
            self.split.clone_from(s);
            Self::normalize_split_list(&mut self.split);
        }

        Self::apply_optional(&mut self.local_names_first, cli.local_names_first);
        Self::apply_optional(&mut self.print_one_name, cli.print_one_name);
        Self::apply_optional(&mut self.print_stations, cli.print_stations);
        Self::apply_optional(
            &mut self.scramble_checker_top_ranked,
            cli.scramble_checker_top_ranked,
        );
        Self::apply_optional(
            &mut self.scramble_checker_final_rounds,
            cli.scramble_checker_final_rounds,
        );
        Self::apply_optional(&mut self.scramble_checker_blank, cli.scramble_checker_blank);
    }

    fn normalize_cover_sheet_list(list: &mut Vec<CoverSheetBy>) {
        list.retain(|c| *c != CoverSheetBy::None);
        list.sort_by_key(|c| c.tier());
        list.dedup();
    }

    fn normalize_split_list(list: &mut Vec<SplitBy>) {
        list.sort_by_key(|s| *s as u8);
        list.dedup();
    }

    /// Validates that PDF file splitting is not more specific than cover sheet criteria.
    pub fn validate_compatibility(&self) -> Result<(), OptionsCompatibilityError> {
        if !self.cover_sheets || self.split.is_empty() {
            return Ok(());
        }

        if self.cover_sheets_by.contains(&CoverSheetBy::Round) {
            if self.split.contains(&SplitBy::Stage) {
                return Err(OptionsCompatibilityError::IncompatibleSplit {
                    split: SplitBy::Stage,
                    cover_sheet: CoverSheetBy::Round,
                });
            }
            if self.split.contains(&SplitBy::Group) {
                return Err(OptionsCompatibilityError::IncompatibleSplit {
                    split: SplitBy::Group,
                    cover_sheet: CoverSheetBy::Round,
                });
            }
        }

        if self.cover_sheets_by.contains(&CoverSheetBy::Group)
            && self.split.contains(&SplitBy::Stage)
        {
            return Err(OptionsCompatibilityError::IncompatibleSplit {
                split: SplitBy::Stage,
                cover_sheet: CoverSheetBy::Group,
            });
        }

        Ok(())
    }

    fn apply_optional<T: Clone>(target: &mut T, src: Option<T>) {
        if let Some(val) = src {
            *target = val;
        }
    }

    /// Formats a human-readable configuration summary table inside a modern card.
    pub fn format_summary(&self) -> String {
        use std::fmt::Write;

        let green_check = "✔".green();
        let red_x = "✖".red();

        let mut lines = Vec::with_capacity(5);
        let mut buf = String::with_capacity(36);
        let _ = write!(buf, "{:28}{}", "Paper Size:", self.paper);
        lines.push(buf);

        let mut buf = String::with_capacity(36);
        let _ = write!(buf, "{:28}{}", "Format:", self.format);
        lines.push(buf);

        // Cover Sheets
        if self.cover_sheets {
            if self.cover_sheets_by.is_empty() {
                let mut buf = String::with_capacity(36);
                let _ = write!(buf, "{:28}{green_check}", "Cover Sheets:");
                lines.push(buf);
            } else {
                let mut buf = String::with_capacity(64);
                let _ = write!(buf, "{:28}{green_check} - ", "Cover Sheets:");
                for (i, by) in self.cover_sheets_by.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    let _ = write!(buf, "{by}");
                }
                lines.push(buf);
            }
        } else {
            let mut buf = String::with_capacity(36);
            let _ = write!(buf, "{:28}{red_x}", "Cover Sheets:");
            lines.push(buf);
        }

        // Split PDFs
        if self.split.is_empty() {
            let mut buf = String::with_capacity(36);
            let _ = write!(buf, "{:28}{red_x}", "Split PDFs:");
            lines.push(buf);
        } else {
            let mut buf = String::with_capacity(64);
            let _ = write!(buf, "{:28}{green_check} - ", "Split PDFs:");
            for (i, split) in self.split.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                let _ = write!(buf, "{split}");
            }
            lines.push(buf);
        }

        // Active Options
        let flags: &[(&str, bool)] = &[
            ("Local Names First", self.local_names_first),
            ("Print One Name", self.print_one_name),
            ("Print Stations", self.print_stations),
            (
                "Scramble Check Top Ranked",
                self.scramble_checker_top_ranked,
            ),
            (
                "Scramble Check Final Rounds",
                self.scramble_checker_final_rounds,
            ),
            ("Scramble Check Blank Cards", self.scramble_checker_blank),
        ];

        let active: Vec<&str> = flags
            .iter()
            .filter_map(|&(name, active)| active.then_some(name))
            .collect();

        if active.is_empty() {
            let mut buf = String::with_capacity(36);
            let _ = write!(buf, "{:28}{red_x}", "Active Options:");
            lines.push(buf);
        } else {
            let mut buf = String::with_capacity(128);
            let _ = write!(buf, "{:28}{green_check} - ", "Active Options:");
            for (i, opt) in active.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                buf.push_str(opt);
            }
            lines.push(buf);
        }

        crate::progress::draw_box("Configuration Summary", &lines)
    }
}

impl std::fmt::Display for ResolvedOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_summary())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing_basic() {
        let args = vec!["scorecard-gen", "Comp2026", "333", "333-2"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.comp_source, Some("Comp2026".to_string()));
        assert_eq!(cli.events, vec!["333", "333-2"]);
    }

    #[test]
    fn test_cli_parsing_flags() {
        let args = vec![
            "scorecard-gen",
            "Comp2026",
            "-p",
            "a4",
            "-f",
            "group",
            "--print-stations",
            "--print-scorecards-cover-sheets",
            "false",
            "-s",
            "event,group",
        ];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.paper, Some(PaperSize::A4));
        assert_eq!(cli.format, Some(PageFormat::Group));
        assert_eq!(cli.print_stations, Some(true));
        assert_eq!(cli.cover_sheets, Some(vec![CoverSheetBy::None]));
        assert_eq!(cli.split, Some(vec![SplitBy::Event, SplitBy::Group]));
    }

    #[test]
    fn test_cli_invalid_paper() {
        let args = vec!["scorecard-gen", "Comp2026", "-p", "tabloid"];
        let result = Cli::try_parse_from(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_invalid_format() {
        let args = vec!["scorecard-gen", "Comp2026", "-f", "unknown_format"];
        let result = Cli::try_parse_from(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_split_parsing() {
        let args = vec!["scorecard-gen", "Comp2026", "-s", "event", "-s", "stage"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.split, Some(vec![SplitBy::Event, SplitBy::Stage]));

        let args_comma = vec!["scorecard-gen", "Comp2026", "--split", "stage,group,event"];
        let cli_comma = Cli::try_parse_from(args_comma).unwrap();
        assert_eq!(
            cli_comma.split,
            Some(vec![SplitBy::Stage, SplitBy::Group, SplitBy::Event])
        );

        // Test backwards-compatible --shard alias
        let args_alias = vec!["scorecard-gen", "Comp2026", "--shard", "event,group"];
        let cli_alias = Cli::try_parse_from(args_alias).unwrap();
        assert_eq!(cli_alias.split, Some(vec![SplitBy::Event, SplitBy::Group]));
    }

    #[test]
    fn test_resolved_options_merge() {
        let cli = Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-p", "letter"]).unwrap();
        let groupifier = GroupifierCompetitionConfig {
            scorecard_paper_size: Some("a4".to_string()),
            print_stations: Some(true),
            scorecard_order: Some("stacked".to_string()),
            ..Default::default()
        };

        let resolved = ResolvedOptions::resolve(&cli, Some(&groupifier));
        // CLI "-p letter" overrides groupifier "a4"
        assert_eq!(resolved.paper, PaperSize::Letter);
        // Groupifier print_stations applies
        assert!(resolved.print_stations);
        // Groupifier stacked order applies
        assert_eq!(resolved.format, PageFormat::Stacked);
        // Default cover_sheets is false
        assert!(!resolved.cover_sheets);
    }

    #[test]
    fn test_cover_sheets_split_behavior() {
        // When cover sheets are enabled without explicit args (-c), defaults to Stage
        let cli_cover = Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-c"]).unwrap();
        let resolved = ResolvedOptions::resolve(&cli_cover, None);
        assert!(resolved.cover_sheets);
        assert!(resolved.split.is_empty());
        assert_eq!(resolved.cover_sheets_by, vec![CoverSheetBy::Stage]);
        assert!(resolved.validate_compatibility().is_ok());

        // When -c is given explicit args r, g, s in any order, it sorts highest tier first (Round -> Group -> Stage)
        let cli_all =
            Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-c", "s,g,r"]).unwrap();
        let resolved_all = ResolvedOptions::resolve(&cli_all, None);
        assert!(resolved_all.cover_sheets);
        assert_eq!(
            resolved_all.cover_sheets_by,
            vec![
                CoverSheetBy::Round,
                CoverSheetBy::Group,
                CoverSheetBy::Stage
            ]
        );

        // When cover sheets are enabled with stage, and file split is stage: compatible
        let cli_override =
            Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-c", "s", "-s", "stage"])
                .unwrap();
        let resolved_override = ResolvedOptions::resolve(&cli_override, None);
        assert!(resolved_override.cover_sheets);
        assert_eq!(resolved_override.split, vec![SplitBy::Stage]);
        assert_eq!(resolved_override.cover_sheets_by, vec![CoverSheetBy::Stage]);
        assert!(resolved_override.validate_compatibility().is_ok());

        // When round cover sheet is given and file split is stage: incompatible!
        let cli_incompatible =
            Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-c", "r", "-s", "stage"])
                .unwrap();
        let resolved_incompatible = ResolvedOptions::resolve(&cli_incompatible, None);
        assert_eq!(
            resolved_incompatible.validate_compatibility(),
            Err(OptionsCompatibilityError::IncompatibleSplit {
                split: SplitBy::Stage,
                cover_sheet: CoverSheetBy::Round,
            })
        );
        let err = resolved_incompatible.validate_compatibility().unwrap_err();
        assert!(
            err.to_string().contains(
                "Splitting PDF files by 'stage' is more specific than 'round' cover sheet"
            )
        );

        // When group cover sheet is given and file split is stage: incompatible!
        let cli_incompatible_group =
            Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-c", "g", "-s", "stage"])
                .unwrap();
        let resolved_incompatible_group = ResolvedOptions::resolve(&cli_incompatible_group, None);
        assert_eq!(
            resolved_incompatible_group.validate_compatibility(),
            Err(OptionsCompatibilityError::IncompatibleSplit {
                split: SplitBy::Stage,
                cover_sheet: CoverSheetBy::Group,
            })
        );

        // When stage cover sheet is given and file split is stage: compatible!
        let cli_custom_ok =
            Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-c", "s", "-s", "stage"])
                .unwrap();
        let resolved_custom_ok = ResolvedOptions::resolve(&cli_custom_ok, None);
        assert!(resolved_custom_ok.validate_compatibility().is_ok());

        // When explicitly disabled with false
        let cli_disabled =
            Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-c", "false"]).unwrap();
        let resolved_disabled = ResolvedOptions::resolve(&cli_disabled, None);
        assert!(!resolved_disabled.cover_sheets);
        assert!(resolved_disabled.cover_sheets_by.is_empty());
    }

    #[test]
    fn test_cover_sheet_by_display_and_parsing() {
        assert_eq!("round".parse::<CoverSheetBy>(), Ok(CoverSheetBy::Round));
        assert_eq!("r".parse::<CoverSheetBy>(), Ok(CoverSheetBy::Round));
        assert_eq!("event".parse::<CoverSheetBy>(), Ok(CoverSheetBy::Round));
        assert_eq!("e".parse::<CoverSheetBy>(), Ok(CoverSheetBy::Round));
        assert_eq!("group".parse::<CoverSheetBy>(), Ok(CoverSheetBy::Group));
        assert_eq!("g".parse::<CoverSheetBy>(), Ok(CoverSheetBy::Group));
        assert_eq!("stage".parse::<CoverSheetBy>(), Ok(CoverSheetBy::Stage));
        assert_eq!("s".parse::<CoverSheetBy>(), Ok(CoverSheetBy::Stage));
        assert_eq!("none".parse::<CoverSheetBy>(), Ok(CoverSheetBy::None));
        assert_eq!("false".parse::<CoverSheetBy>(), Ok(CoverSheetBy::None));
        assert!("invalid".parse::<CoverSheetBy>().is_err());

        assert_eq!(CoverSheetBy::Round.to_string(), "round");
        assert_eq!(CoverSheetBy::Group.to_string(), "group");
        assert_eq!(CoverSheetBy::Stage.to_string(), "stage");
    }

    #[test]
    fn test_split_by_display_and_parsing() {
        assert_eq!("event".parse::<SplitBy>(), Ok(SplitBy::Event));
        assert_eq!("events".parse::<SplitBy>(), Ok(SplitBy::Event));
        assert_eq!("group".parse::<SplitBy>(), Ok(SplitBy::Group));
        assert_eq!("groups".parse::<SplitBy>(), Ok(SplitBy::Group));
        assert_eq!("stage".parse::<SplitBy>(), Ok(SplitBy::Stage));
        assert_eq!("stages".parse::<SplitBy>(), Ok(SplitBy::Stage));
        assert_eq!("room".parse::<SplitBy>(), Ok(SplitBy::Stage));
        assert_eq!("rooms".parse::<SplitBy>(), Ok(SplitBy::Stage));
        assert!("invalid".parse::<SplitBy>().is_err());

        assert_eq!(SplitBy::Event.to_string(), "event");
        assert_eq!(SplitBy::Group.to_string(), "group");
        assert_eq!(SplitBy::Stage.to_string(), "stage");
    }

    #[test]
    fn test_resolved_options_boolean_flag_overrides() {
        let cli = Cli::try_parse_from(vec![
            "scorecard-gen",
            "Comp2026",
            "--print-scorecards-cover-sheets",
            "false",
            "--local-names-first",
            "true",
            "--print-one-name",
            "true",
            "--print-stations",
            "false",
        ])
        .unwrap();

        let groupifier = GroupifierCompetitionConfig {
            print_scorecards_cover_sheets: Some(true),
            print_stations: Some(true),
            local_names_first: Some(false),
            ..Default::default()
        };

        let resolved = ResolvedOptions::resolve(&cli, Some(&groupifier));
        assert!(!resolved.cover_sheets);
        assert!(resolved.local_names_first);
        assert!(resolved.print_one_name);
        assert!(!resolved.print_stations);
    }

    #[test]
    fn test_resolved_options_format_summary() {
        let opts_default = ResolvedOptions::default();
        let summary_default = opts_default.to_string();
        assert!(summary_default.contains("Configuration Summary"));
        assert!(summary_default.contains("Cover Sheets:"));
        assert!(summary_default.contains("Split PDFs:"));
        assert!(summary_default.contains("Active Options:"));
        assert!(summary_default.contains('✖'));

        let opts_custom = ResolvedOptions {
            cover_sheets: true,
            cover_sheets_by: vec![CoverSheetBy::Stage, CoverSheetBy::Round],
            split: vec![SplitBy::Event, SplitBy::Stage],
            print_stations: true,
            ..Default::default()
        };
        let summary_custom = opts_custom.format_summary();
        assert!(summary_custom.contains("Configuration Summary"));
        assert!(summary_custom.contains("Cover Sheets:"));
        assert!(summary_custom.contains("stage, round"));
        assert!(summary_custom.contains("Split PDFs:"));
        assert!(summary_custom.contains("event, stage"));
        assert!(summary_custom.contains("Active Options:"));
        assert!(summary_custom.contains("Print Stations"));
        assert!(summary_custom.contains('✔'));
    }
}
