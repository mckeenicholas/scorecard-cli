use crate::pdf::{PageFormat, PaperSize};
use crate::wcif::GroupifierCompetitionConfig;
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

/// Sharding dimension for splitting output PDFs into separate files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShardBy {
    #[value(name = "event", alias = "events", alias = "e")]
    Event,
    #[value(name = "group", alias = "groups", alias = "g")]
    Group,
    #[value(name = "stage", alias = "stages", alias = "room", alias = "rooms")]
    Stage,
}

impl std::fmt::Display for ShardBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShardBy::Event => write!(f, "event"),
            ShardBy::Group => write!(f, "group"),
            ShardBy::Stage => write!(f, "stage"),
        }
    }
}

impl std::str::FromStr for ShardBy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "event" | "events" | "e" => Ok(ShardBy::Event),
            "group" | "groups" | "g" => Ok(ShardBy::Group),
            "stage" | "stages" | "room" | "rooms" | "s" => Ok(ShardBy::Stage),
            other => Err(format!(
                "invalid shard criterion '{}': must be 'event', 'group', or 'stage'",
                other
            )),
        }
    }
}

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
        long = "shard",
        value_enum,
        value_name = "SHARD",
        value_delimiter = ',',
        num_args = 1..
    )]
    pub shard: Option<Vec<ShardBy>>,

    /// Don't include unicode characters
    #[arg(short = 'a', long)]
    pub ascii: bool,

    /// Include/exclude cover sheets
    #[arg(
        short = 'c',
        long,
        visible_alias = "print-scorecards-cover-sheets",
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub cover_sheets: Option<bool>,

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

/// Fully resolved scorecard generation options after merging CLI flags,
/// WCIF Groupifier config extensions, and default values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOptions {
    pub paper: PaperSize,
    pub format: PageFormat,
    pub ascii: bool,
    pub cover_sheets: bool,
    pub shard: Vec<ShardBy>,
    pub local_names_first: bool,             // TODO: wire to renderer
    pub print_one_name: bool,                // TODO: wire to renderer
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
            ascii: false,
            cover_sheets: true,
            shard: Vec::new(),
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
        Self::apply_optional(&mut self.cover_sheets, cfg.print_scorecards_cover_sheets);
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
        self.ascii = cli.ascii;
        Self::apply_optional(&mut self.cover_sheets, cli.cover_sheets);

        if let Some(ref s) = cli.shard {
            self.shard = s.clone();
            self.normalize_shards();
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

    fn normalize_shards(&mut self) {
        self.shard.sort_by_key(|s| *s as u8);
        self.shard.dedup();
    }

    fn apply_optional<T: Clone>(target: &mut T, src: Option<T>) {
        if let Some(val) = src {
            *target = val;
        }
    }

    /// Formats a human-readable configuration summary table.
    pub fn format_summary(&self) -> String {
        let mut out = String::new();
        out.push_str("\n--- Configuration Summary ---\n");
        out.push_str(&format!("Paper Size:                  {}\n", self.paper));
        out.push_str(&format!("Format:                      {}\n", self.format));
        out.push_str(&format!("ASCII Only:                  {}\n", self.ascii));
        out.push_str(&format!("Cover Sheets:                {}\n", self.cover_sheets));
        if self.shard.is_empty() {
            out.push_str("Shard By:                    (None - Single PDF)\n");
        } else {
            let shard_strs: Vec<String> = self.shard.iter().map(|s| s.to_string()).collect();
            out.push_str(&format!("Shard By:                    {}\n", shard_strs.join(", ")));
        }
        out.push_str(&format!("Local Names First:           {}\n", self.local_names_first));
        out.push_str(&format!("Print One Name:              {}\n", self.print_one_name));
        out.push_str(&format!("Print Stations (Station #):  {}\n", self.print_stations));
        out.push_str(&format!(
            "Scramble Chk Top Ranked:     {}\n",
            self.scramble_checker_top_ranked
        ));
        out.push_str(&format!(
            "Scramble Chk Final Rounds:   {}\n",
            self.scramble_checker_final_rounds
        ));
        out.push_str(&format!(
            "Scramble Chk Blank Cards:    {}\n",
            self.scramble_checker_blank
        ));
        out.push_str("-----------------------------");
        out
    }

    /// Prints the human-readable configuration summary table to stdout.
    pub fn print_summary(&self) {
        println!("{}", self.format_summary());
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
            "--print-stations",
            "--print-scorecards-cover-sheets",
            "false",
            "-s",
            "event,group",
        ];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.paper, Some(PaperSize::A4));
        assert_eq!(cli.format, Some(PageFormat::Group));
        assert!(cli.ascii);
        assert_eq!(cli.print_stations, Some(true));
        assert_eq!(cli.cover_sheets, Some(false));
        assert_eq!(cli.shard, Some(vec![ShardBy::Event, ShardBy::Group]));
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
    fn test_cli_shard_parsing() {
        let args = vec!["scorecard-gen", "Comp2026", "-s", "event", "-s", "stage"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.shard, Some(vec![ShardBy::Event, ShardBy::Stage]));

        let args_comma = vec!["scorecard-gen", "Comp2026", "--shard", "stage,group,event"];
        let cli_comma = Cli::try_parse_from(args_comma).unwrap();
        assert_eq!(
            cli_comma.shard,
            Some(vec![ShardBy::Stage, ShardBy::Group, ShardBy::Event])
        );
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
        // Default cover_sheets remains true
        assert!(resolved.cover_sheets);
    }

    #[test]
    fn test_shard_by_display_and_parsing() {
        assert_eq!("event".parse::<ShardBy>(), Ok(ShardBy::Event));
        assert_eq!("events".parse::<ShardBy>(), Ok(ShardBy::Event));
        assert_eq!("group".parse::<ShardBy>(), Ok(ShardBy::Group));
        assert_eq!("groups".parse::<ShardBy>(), Ok(ShardBy::Group));
        assert_eq!("stage".parse::<ShardBy>(), Ok(ShardBy::Stage));
        assert_eq!("stages".parse::<ShardBy>(), Ok(ShardBy::Stage));
        assert_eq!("room".parse::<ShardBy>(), Ok(ShardBy::Stage));
        assert_eq!("rooms".parse::<ShardBy>(), Ok(ShardBy::Stage));
        assert!("invalid".parse::<ShardBy>().is_err());

        assert_eq!(ShardBy::Event.to_string(), "event");
        assert_eq!(ShardBy::Group.to_string(), "group");
        assert_eq!(ShardBy::Stage.to_string(), "stage");
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
}
