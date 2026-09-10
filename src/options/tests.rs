use clap::Parser;

use super::cli::Cli;
use super::resolved::ResolvedOptions;
use super::types::{CoverSheetBy, OptionsCompatibilityError, SplitBy};
use crate::pdf::{PageFormat, PaperSize};
use crate::wcif::GroupifierCompetitionConfig;

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
    let cli_all = Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-c", "s,g,r"]).unwrap();
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
        Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-c", "s", "-s", "stage"]).unwrap();
    let resolved_override = ResolvedOptions::resolve(&cli_override, None);
    assert!(resolved_override.cover_sheets);
    assert_eq!(resolved_override.split, vec![SplitBy::Stage]);
    assert_eq!(resolved_override.cover_sheets_by, vec![CoverSheetBy::Stage]);
    assert!(resolved_override.validate_compatibility().is_ok());

    // When round cover sheet is given and file split is stage: incompatible!
    let cli_incompatible =
        Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-c", "r", "-s", "stage"]).unwrap();
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
        err.to_string()
            .contains("Splitting PDF files by 'stage' is more specific than 'round' cover sheet")
    );

    // When group cover sheet is given and file split is stage: incompatible!
    let cli_incompatible_group =
        Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-c", "g", "-s", "stage"]).unwrap();
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
        Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "-c", "s", "-s", "stage"]).unwrap();
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

#[test]
fn test_cli_start_group_on_new_page_flag() {
    let cli = Cli::try_parse_from(vec![
        "scorecard-gen",
        "Comp2026",
        "--start-group-on-new-page",
    ])
    .unwrap();
    assert_eq!(cli.start_group_on_new_page, Some(true));

    let resolved = ResolvedOptions::resolve(&cli, None);
    assert!(resolved.start_group_on_new_page);

    let summary = resolved.format_summary();
    assert!(summary.contains("Start Group on New Page"));

    // Test alias
    let cli_alias =
        Cli::try_parse_from(vec!["scorecard-gen", "Comp2026", "--group-new-page"]).unwrap();
    assert_eq!(cli_alias.start_group_on_new_page, Some(true));
}
