use super::cli::Cli;
use super::types::{CoverSheetBy, OptionsCompatibilityError, SplitBy};
use crate::pdf::{PageFormat, PaperSize};
use crate::wcif::GroupifierCompetitionConfig;
use crossterm::style::Stylize;
use std::path::PathBuf;

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
    pub start_group_on_new_page: bool,
    pub font: Option<PathBuf>,
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
            start_group_on_new_page: false,
            font: None,
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
        Self::apply_optional(
            &mut self.start_group_on_new_page,
            cfg.start_group_on_new_page,
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
        Self::apply_optional(
            &mut self.start_group_on_new_page,
            cli.start_group_on_new_page,
        );
        if let Some(ref f) = cli.font {
            self.font = Some(f.clone());
        }
    }

    fn normalize_cover_sheet_list(list: &mut Vec<CoverSheetBy>) {
        list.retain(|c| *c != CoverSheetBy::None);
        list.sort_by_key(|c| c.tier());
        list.dedup();
    }

    fn normalize_split_list(list: &mut Vec<SplitBy>) {
        list.sort();
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
            ("Start Group on New Page", self.start_group_on_new_page),
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

        let mut active = flags
            .iter()
            .filter_map(|&(name, active)| active.then_some(name));

        if let Some(first) = active.next() {
            let mut buf = String::with_capacity(128);
            let _ = write!(buf, "{:28}{green_check} - {first}", "Active Options:");
            for opt in active {
                buf.push_str(", ");
                buf.push_str(opt);
            }
            lines.push(buf);
        } else {
            let mut buf = String::with_capacity(36);
            let _ = write!(buf, "{:28}{red_x}", "Active Options:");
            lines.push(buf);
        }

        if let Some(ref path) = self.font {
            let mut buf = String::with_capacity(64);
            let _ = write!(buf, "{:28}{}", "Font:", path.display());
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
