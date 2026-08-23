use crate::wcif::GroupifierCompetitionConfig;
use clap::Parser;

/// Fast WCA Cubing Competition Scorecard Generator in Rust
#[derive(Parser, Debug, Clone)]
#[command(
    name = "scorecard-gen",
    about = "Fast cubing competition scorecard generator in Rust",
    version
)]
pub struct Cli {
    /// Competition name/ID or WCIF file path
    #[arg(value_name = "COMPETITION")]
    pub comp_source: String,

    /// Events and rounds (e.g. 333, 333-2, 333-r2)
    #[arg(value_name = "EVENTS")]
    pub events: Vec<String>,

    /// Set paper size (a4, letter, a6) [overrides WCIF]
    #[arg(
        short = 'p',
        long,
        visible_alias = "scorecard-paper-size",
        value_name = "PAPER"
    )]
    pub paper: Option<String>,

    /// Set page layout format (group, stacked)
    #[arg(short = 'f', long, value_name = "FORMAT")]
    pub format: Option<String>,

    /// Don't include CJK or accents (ASCII only)
    #[arg(short = 'a', long)]
    pub ascii: bool,

    /// Include/exclude cover sheets [true|false]
    #[arg(
        short = 'c',
        long,
        visible_alias = "print-scorecards-cover-sheets",
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub cover_sheets: Option<bool>,

    /// Set scorecard sorting order (natural, etc.)
    #[arg(short = 'o', long, value_name = "ORDER")]
    pub scorecard_order: Option<String>,

    /// Display local names first [true|false]
    #[arg(
        short = 'l',
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub local_names_first: Option<bool>,

    /// Only print one name [true|false]
    #[arg(
        short = 'n',
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub print_one_name: Option<bool>,

    /// Print station numbers [true|false]
    #[arg(
        short = 's',
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub print_stations: Option<bool>,

    /// Print scramble checker for top ranked competitors [true|false]
    #[arg(
        short = 't',
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub scramble_checker_top_ranked: Option<bool>,

    /// Print scramble checker for final rounds [true|false]
    #[arg(
        short = 'r',
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub scramble_checker_final_rounds: Option<bool>,

    /// Print scramble checker for blank scorecards [true|false]
    #[arg(
        short = 'b',
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub scramble_checker_blank: Option<bool>,
}

impl Cli {
    /// Validates the CLI options and returns normalized paper & format values if valid.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ref p) = self.paper {
            let lower = p.to_lowercase();
            if lower != "a4" && lower != "letter" && lower != "a6" {
                return Err(format!(
                    "invalid paper size: {:?} (must be a4, letter, or a6)",
                    p
                ));
            }
        }

        if let Some(ref f) = self.format {
            let lower = f.to_lowercase();
            if lower != "group" && lower != "stacked" {
                return Err(format!(
                    "invalid format: {:?} (must be group or stacked)",
                    f
                ));
            }
        }

        Ok(())
    }
}

/// Fully resolved scorecard generation options after merging CLI flags,
/// WCIF Groupifier config extensions, and default values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOptions {
    pub paper: String,
    pub format: String,
    pub ascii: bool,
    pub cover_sheets: bool,
    pub scorecard_order: String,
    pub local_names_first: bool,
    pub print_one_name: bool,
    pub print_stations: bool,
    pub scramble_checker_top_ranked: bool,
    pub scramble_checker_final_rounds: bool,
    pub scramble_checker_blank: bool,
}

impl Default for ResolvedOptions {
    fn default() -> Self {
        Self {
            paper: "letter".to_string(),
            format: "group".to_string(),
            ascii: false,
            cover_sheets: true,
            scorecard_order: "natural".to_string(),
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

        fn apply<T: Clone>(target: &mut T, src: Option<T>) {
            if let Some(val) = src {
                *target = val;
            }
        }

        // Layer 1: WCIF Groupifier config
        if let Some(cfg) = groupifier {
            apply(
                &mut opts.paper,
                cfg.scorecard_paper_size.clone().filter(|s| !s.is_empty()),
            );
            apply(&mut opts.cover_sheets, cfg.print_scorecards_cover_sheets);
            apply(
                &mut opts.scorecard_order,
                cfg.scorecard_order.clone().filter(|s| !s.is_empty()),
            );
            apply(&mut opts.local_names_first, cfg.local_names_first);
            apply(&mut opts.print_one_name, cfg.print_one_name);
            apply(&mut opts.print_stations, cfg.print_stations);
            apply(
                &mut opts.scramble_checker_top_ranked,
                cfg.print_scramble_checker_for_top_ranked_competitors,
            );
            apply(
                &mut opts.scramble_checker_final_rounds,
                cfg.print_scramble_checker_for_final_rounds,
            );
            apply(
                &mut opts.scramble_checker_blank,
                cfg.print_scramble_checker_for_blank_scorecards,
            );
        }

        // Layer 2: Explicit CLI flag overrides
        apply(&mut opts.paper, cli.paper.clone());
        apply(&mut opts.format, cli.format.clone());
        opts.ascii = cli.ascii;
        apply(&mut opts.cover_sheets, cli.cover_sheets);
        apply(&mut opts.scorecard_order, cli.scorecard_order.clone());
        apply(&mut opts.local_names_first, cli.local_names_first);
        apply(&mut opts.print_one_name, cli.print_one_name);
        apply(&mut opts.print_stations, cli.print_stations);
        apply(
            &mut opts.scramble_checker_top_ranked,
            cli.scramble_checker_top_ranked,
        );
        apply(
            &mut opts.scramble_checker_final_rounds,
            cli.scramble_checker_final_rounds,
        );
        apply(&mut opts.scramble_checker_blank, cli.scramble_checker_blank);

        opts
    }

    /// Prints a human-readable configuration summary table.
    pub fn print_summary(&self) {
        println!("\n--- Configuration Summary ---");
        println!("Paper Size:                  {}", self.paper.to_uppercase());
        println!("Format:                      {}", self.format);
        println!("ASCII Only:                  {}", self.ascii);
        println!("Cover Sheets:                {}", self.cover_sheets);
        println!("Scorecard Order:             {}", self.scorecard_order);
        println!("Local Names First:           {}", self.local_names_first);
        println!("Print One Name:              {}", self.print_one_name);
        println!("Print Stations (Station #):  {}", self.print_stations);
        println!(
            "Scramble Chk Top Ranked:     {}",
            self.scramble_checker_top_ranked
        );
        println!(
            "Scramble Chk Final Rounds:   {}",
            self.scramble_checker_final_rounds
        );
        println!(
            "Scramble Chk Blank Cards:    {}",
            self.scramble_checker_blank
        );
        println!("-----------------------------");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing_basic() {
        let args = vec!["scorecard-gen", "Comp2026", "333", "333-2"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.comp_source, "Comp2026");
        assert_eq!(cli.events, vec!["333", "333-2"]);
        assert!(cli.validate().is_ok());
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
            "-a",
            "-s",
            "--print-scorecards-cover-sheets",
            "false",
        ];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.paper, Some("a4".to_string()));
        assert_eq!(cli.format, Some("group".to_string()));
        assert!(cli.ascii);
        assert_eq!(cli.print_stations, Some(true));
        assert_eq!(cli.cover_sheets, Some(false));
        assert!(cli.validate().is_ok());
    }

    #[test]
    fn test_cli_invalid_paper() {
        let args = vec!["scorecard-gen", "Comp2026", "-p", "tabloid"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(cli.validate().is_err());
    }

    #[test]
    fn test_resolved_options_merge() {
        let cli = Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-p", "letter"]).unwrap();
        let groupifier = GroupifierCompetitionConfig {
            scorecard_paper_size: Some("a4".to_string()),
            print_stations: Some(true),
            ..Default::default()
        };

        let resolved = ResolvedOptions::resolve(&cli, Some(&groupifier));
        // CLI "-p letter" overrides groupifier "a4"
        assert_eq!(resolved.paper, "letter");
        // Groupifier print_stations applies
        assert!(resolved.print_stations);
        // Default cover_sheets remains true
        assert!(resolved.cover_sheets);
    }
}
