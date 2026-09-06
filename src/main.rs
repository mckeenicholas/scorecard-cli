mod interactive;
mod options;
mod pdf;
mod progress;
mod scorecard;
mod wcif;

use clap::Parser;
use mimalloc::MiMalloc;
use options::{Cli, ResolvedOptions, SplitBy};
use pdf::{PageLayout, PdfGenerationError, PdfGenerator};
use scorecard::{PlannerError, ScorecardItem, ScorecardPlanner, WcaEvent};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::BufWriter;
use std::process;
use wcif::{WcifLoadError, WcifLoader};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const BUFFER_SIZE: usize = 16 * 1024 * 1024; // 16 MB

fn slugify(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut slug = String::with_capacity(lower.len());
    for c in lower.chars() {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if (c.is_whitespace() || c == '-' || c == '_')
            && !slug.ends_with('-')
            && !slug.is_empty()
        {
            slug.push('-');
        }
    }
    slug.trim_end_matches('-').to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SplitKey {
    stage: Option<String>,
    event: Option<(WcaEvent, usize)>,
    group: Option<usize>,
}

impl SplitKey {
    fn to_filename(&self, comp_id: &str) -> String {
        let mut name = format!("{comp_id}-scorecards");
        if let Some(ref stage) = self.stage {
            name.push('-');
            name.push_str(stage);
        }
        if let Some((event, round_number)) = self.event {
            let event_id = event.code();
            let _ = write!(name, "-{event_id}-r{round_number}");
        }
        if let Some(group) = self.group {
            let _ = write!(name, "-group{group}");
        }
        name.push_str(".pdf");
        name
    }
}

fn build_split_key(
    card: &ScorecardItem<'_>,
    has_stage: bool,
    has_event: bool,
    has_group: bool,
) -> SplitKey {
    SplitKey {
        stage: if has_stage {
            Some(slugify(card.stage_name.unwrap_or("no-stage")))
        } else {
            None
        },
        event: if has_event {
            Some((card.event, card.round_number))
        } else {
            None
        },
        group: if has_group {
            Some(card.group_number)
        } else {
            None
        },
    }
}

fn partition_scorecards<'a>(
    comp_id: &str,
    cards: &'a [ScorecardItem<'a>],
    split_by: &[SplitBy],
) -> Vec<(String, Vec<ScorecardItem<'a>>)> {
    if split_by.is_empty() {
        let out_filename = format!("{comp_id}-scorecards.pdf");
        return vec![(out_filename, cards.to_vec())];
    }

    let has_stage = split_by.contains(&SplitBy::Stage);
    let has_event = split_by.contains(&SplitBy::Event);
    let has_group = split_by.contains(&SplitBy::Group);

    cards
        .iter()
        .fold(
            BTreeMap::<SplitKey, Vec<ScorecardItem<'a>>>::new(),
            |mut acc, card| {
                let key = build_split_key(card, has_stage, has_event, has_group);
                acc.entry(key).or_default().push(*card);
                acc
            },
        )
        .into_iter()
        .map(|(k, v)| (k.to_filename(comp_id), v))
        .collect()
}

fn write_pdf_file(
    generator: &PdfGenerator,
    comp: &wcif::Competition,
    filename: &str,
    cards: &[ScorecardItem<'_>],
) -> Result<usize, AppError> {
    let file = File::create(filename).map_err(|source| AppError::CreatePdf {
        path: filename.to_string(),
        source,
    })?;
    let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);

    let spinner = progress::create_spinner(format!(
        "Rendering {} scorecards to PDF ({})...",
        cards.len(),
        filename
    ));

    let page_count = match generator.generate_to_writer(comp, cards, &mut writer) {
        Ok(count) => count,
        Err(source) => {
            spinner.finish_and_clear();
            return Err(AppError::GeneratePdf {
                path: filename.to_string(),
                source,
            });
        }
    };
    spinner.finish_and_clear();
    Ok(page_count)
}

fn load_competition(source: &str) -> Result<wcif::Competition, wcif::WcifLoadError> {
    let spinner = progress::create_spinner(format!("Loading WCIF from: {source}..."));
    let comp = WcifLoader::load(source);
    spinner.finish_and_clear();
    let comp = comp?;
    println!("✔ Loaded competition: {} ({})", comp.name, comp.id);
    Ok(comp)
}

fn resolve_options(cli: &Cli, comp: &wcif::Competition) -> ResolvedOptions {
    let groupifier_config = comp.get_groupifier_config();
    ResolvedOptions::resolve(cli, groupifier_config.as_ref())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BundleKey<'a> {
    stage_name: Option<&'a str>,
    event: WcaEvent,
    round_number: usize,
    group_number: usize,
}

/// Error returned when scorecard file splitting configuration is incompatible with cover sheets.
#[derive(Debug)]
pub enum SplitError {
    IncompatibleOptions(options::OptionsCompatibilityError),
    SplitBundle {
        event_id: String,
        round_number: usize,
        group_number: usize,
        first_file: String,
        second_file: String,
    },
}

impl std::fmt::Display for SplitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SplitError::IncompatibleOptions(e) => write!(f, "{e}"),
            SplitError::SplitBundle {
                event_id,
                round_number,
                group_number,
                first_file,
                second_file,
            } => {
                write!(
                    f,
                    "Incompatible configuration: Scorecard set for event {event_id} round {round_number} group {group_number} is split across files '{first_file}' and '{second_file}'. Cover sheets require each bundle to remain intact in a single file."
                )
            }
        }
    }
}

impl std::error::Error for SplitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SplitError::IncompatibleOptions(e) => Some(e),
            SplitError::SplitBundle { .. } => None,
        }
    }
}

impl From<options::OptionsCompatibilityError> for SplitError {
    fn from(err: options::OptionsCompatibilityError) -> Self {
        SplitError::IncompatibleOptions(err)
    }
}

/// Top-level application error encompassing all potential failure modes.
#[derive(Debug)]
pub enum AppError {
    Wcif(WcifLoadError),
    Interactive(interactive::InteractiveError),
    Planner(PlannerError),
    Split(SplitError),
    CreatePdf {
        path: String,
        source: std::io::Error,
    },
    GeneratePdf {
        path: String,
        source: PdfGenerationError,
    },
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Wcif(e) => write!(f, "{e}"),
            AppError::Interactive(e) => write!(f, "{e}"),
            AppError::Planner(e) => write!(f, "{e}"),
            AppError::Split(e) => write!(f, "{e}"),
            AppError::CreatePdf { path, source } => {
                write!(f, "Error creating output PDF file {path}: {source}")
            }
            AppError::GeneratePdf { path, source } => {
                write!(f, "Error generating scorecards PDF {path}: {source}")
            }
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Wcif(e) => Some(e),
            AppError::Interactive(e) => Some(e),
            AppError::Planner(e) => Some(e),
            AppError::Split(e) => Some(e),
            AppError::CreatePdf { source, .. } => Some(source),
            AppError::GeneratePdf { source, .. } => Some(source),
        }
    }
}

impl From<WcifLoadError> for AppError {
    fn from(err: WcifLoadError) -> Self {
        AppError::Wcif(err)
    }
}

impl From<interactive::InteractiveError> for AppError {
    fn from(err: interactive::InteractiveError) -> Self {
        AppError::Interactive(err)
    }
}

impl From<PlannerError> for AppError {
    fn from(err: PlannerError) -> Self {
        AppError::Planner(err)
    }
}

impl From<SplitError> for AppError {
    fn from(err: SplitError) -> Self {
        AppError::Split(err)
    }
}

fn validate_card_bundle_placement(
    partitions: &[(String, Vec<ScorecardItem<'_>>)],
) -> Result<(), SplitError> {
    let mut bundle_partition_map: std::collections::HashMap<BundleKey<'_>, &str> =
        std::collections::HashMap::new();

    for (filename, partition_cards) in partitions {
        for card in partition_cards {
            if card.is_cover_sheet {
                continue;
            }
            let bundle_key = BundleKey {
                stage_name: card.stage_name,
                event: card.event,
                round_number: card.round_number,
                group_number: card.group_number,
            };
            if let Some(existing_file) = bundle_partition_map.get(&bundle_key) {
                if *existing_file != filename.as_str() {
                    return Err(SplitError::SplitBundle {
                        event_id: card.event.code().to_string(),
                        round_number: card.round_number,
                        group_number: card.group_number,
                        first_file: existing_file.to_string(),
                        second_file: filename.clone(),
                    });
                }
            } else {
                bundle_partition_map.insert(bundle_key, filename.as_str());
            }
        }
    }

    Ok(())
}

fn validate_split_compatibility(
    partitions: &[(String, Vec<ScorecardItem<'_>>)],
    options: &ResolvedOptions,
) -> Result<(), SplitError> {
    options.validate_compatibility()?;

    if options.cover_sheets {
        validate_card_bundle_placement(partitions)?;
    }

    Ok(())
}

fn write_and_report_partition(
    generator: &PdfGenerator,
    comp: &wcif::Competition,
    out_filename: &str,
    partition_cards: &[ScorecardItem<'_>],
    is_multi: bool,
) -> Result<usize, AppError> {
    let page_count = write_pdf_file(generator, comp, out_filename, partition_cards)?;
    let icon = "✔";
    let scorecards_word = if partition_cards.len() == 1 {
        "scorecard"
    } else {
        "scorecards"
    };
    let pages_word = if page_count == 1 { "page" } else { "pages" };
    if is_multi {
        println!(
            "  {icon} Generated {} ({} {}, {} {})",
            out_filename,
            partition_cards.len(),
            scorecards_word,
            page_count,
            pages_word
        );
    } else {
        println!(
            "{icon} Successfully generated scorecards PDF: {} ({} {}, {} {})",
            out_filename,
            partition_cards.len(),
            scorecards_word,
            page_count,
            pages_word
        );
    }
    Ok(page_count)
}

fn generate_partitioned_pdfs(
    comp: &wcif::Competition,
    cards: &[ScorecardItem<'_>],
    options: &ResolvedOptions,
) -> Result<(), AppError> {
    let layout = PageLayout::new(options.paper);
    let generator = PdfGenerator::with_format(layout, options.format);
    let partitions = partition_scorecards(&comp.id, cards, &options.split);
    validate_split_compatibility(&partitions, options)?;
    let is_multi = partitions.len() > 1;

    if is_multi {
        println!("Generating {} separate PDF files...", partitions.len());
    }

    let mut total_pages = 0;
    for (out_filename, partition_cards) in &partitions {
        let pages =
            write_and_report_partition(&generator, comp, out_filename, partition_cards, is_multi)?;
        total_pages += pages;
    }

    if is_multi {
        let pages_word = if total_pages == 1 { "page" } else { "pages" };
        println!(
            "\n✔ Successfully generated all {} separate PDF files ({} {} total).",
            partitions.len(),
            total_pages,
            pages_word
        );
    }

    Ok(())
}

fn init_cli_and_competition() -> Result<(Cli, wcif::Competition), AppError> {
    if std::env::args().len() <= 1 {
        Ok(interactive::prompt_interactive_flow()?)
    } else {
        let parsed = Cli::parse();
        match parsed.comp_source.as_deref() {
            Some(source) => {
                let comp = load_competition(source)?;
                Ok((parsed, comp))
            }
            None => Ok(interactive::prompt_interactive_flow()?),
        }
    }
}

fn run() -> Result<(), AppError> {
    let (cli, comp) = init_cli_and_competition()?;

    let active_opts = resolve_options(&cli, &comp);
    println!("\n{active_opts}");

    let plan = ScorecardPlanner::plan(
        &comp,
        &cli.events,
        active_opts.cover_sheets,
        &active_opts.cover_sheets_by,
        active_opts.print_stations,
        active_opts.print_one_name,
    )?;

    if plan.is_empty() {
        println!("No scorecards to generate.");
        return Ok(());
    }

    println!("{plan}");
    generate_partitioned_pdfs(&comp, &plan.items, &active_opts)?;

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Red Stage"), "red-stage");
        assert_eq!(slugify("Main Stage (Room 101)"), "main-stage-room-101");
        assert_eq!(slugify("3x3x3 Cube"), "3x3x3-cube");
        assert_eq!(slugify("  --Extra   Spaces-- "), "extra-spaces");
    }

    #[test]
    fn test_partition_scorecards_none() {
        let cards = vec![
            ScorecardItem {
                scorecard_number: 1,
                station_number: Some(1),
                competition_name: "Comp",
                event: WcaEvent::E333,
                round_number: 1,
                group_number: 1,
                stage_name: Some("Red Stage"),
                competitor_name: "Alice",
                registrant_id: Some(1),
                wca_id: None,
                attempt_count: 5,
                time_limit_info: None,
                is_blank: false,
                is_cover_sheet: false,
                total_group_cards: 0,
            },
            ScorecardItem {
                scorecard_number: 2,
                station_number: Some(2),
                competition_name: "Comp",
                event: WcaEvent::E222,
                round_number: 1,
                group_number: 2,
                stage_name: Some("Blue Stage"),
                competitor_name: "Bob",
                registrant_id: Some(2),
                wca_id: None,
                attempt_count: 5,
                time_limit_info: None,
                is_blank: false,
                is_cover_sheet: false,
                total_group_cards: 0,
            },
        ];

        let partitions = partition_scorecards("Comp2026", &cards, &[]);
        assert_eq!(partitions.len(), 1);
        assert_eq!(partitions[0].0, "Comp2026-scorecards.pdf");
        assert_eq!(partitions[0].1.len(), 2);
    }

    #[test]
    fn test_partition_scorecards_by_event() {
        let cards = vec![
            ScorecardItem {
                scorecard_number: 1,
                station_number: Some(1),
                competition_name: "Comp",
                event: WcaEvent::E333,
                round_number: 1,
                group_number: 1,
                stage_name: Some("Red Stage"),
                competitor_name: "Alice",
                registrant_id: Some(1),
                wca_id: None,
                attempt_count: 5,
                time_limit_info: None,
                is_blank: false,
                is_cover_sheet: false,
                total_group_cards: 0,
            },
            ScorecardItem {
                scorecard_number: 2,
                station_number: Some(2),
                competition_name: "Comp",
                event: WcaEvent::E222,
                round_number: 1,
                group_number: 2,
                stage_name: Some("Blue Stage"),
                competitor_name: "Bob",
                registrant_id: Some(2),
                wca_id: None,
                attempt_count: 5,
                time_limit_info: None,
                is_blank: false,
                is_cover_sheet: false,
                total_group_cards: 0,
            },
        ];

        let partitions = partition_scorecards("Comp2026", &cards, &[SplitBy::Event]);
        assert_eq!(partitions.len(), 2);
        assert_eq!(partitions[0].0, "Comp2026-scorecards-333-r1.pdf");
        assert_eq!(partitions[1].0, "Comp2026-scorecards-222-r1.pdf");
    }

    #[test]
    fn test_partition_scorecards_by_all_three() {
        let cards = vec![
            ScorecardItem {
                scorecard_number: 1,
                station_number: Some(1),
                competition_name: "Comp",
                event: WcaEvent::E333,
                round_number: 1,
                group_number: 1,
                stage_name: Some("Red Stage"),
                competitor_name: "Alice",
                registrant_id: Some(1),
                wca_id: None,
                attempt_count: 5,
                time_limit_info: None,
                is_blank: false,
                is_cover_sheet: false,
                total_group_cards: 0,
            },
            ScorecardItem {
                scorecard_number: 2,
                station_number: Some(2),
                competition_name: "Comp",
                event: WcaEvent::E333,
                round_number: 1,
                group_number: 2,
                stage_name: Some("Red Stage"),
                competitor_name: "Bob",
                registrant_id: Some(2),
                wca_id: None,
                attempt_count: 5,
                time_limit_info: None,
                is_blank: false,
                is_cover_sheet: false,
                total_group_cards: 0,
            },
        ];

        let partitions = partition_scorecards(
            "Comp2026",
            &cards,
            &[SplitBy::Stage, SplitBy::Event, SplitBy::Group],
        );
        assert_eq!(partitions.len(), 2);
        assert_eq!(
            partitions[0].0,
            "Comp2026-scorecards-red-stage-333-r1-group1.pdf"
        );
        assert_eq!(
            partitions[1].0,
            "Comp2026-scorecards-red-stage-333-r1-group2.pdf"
        );
    }

    #[test]
    fn test_validate_split_compatibility() {
        use options::CoverSheetBy;
        let card1 = ScorecardItem {
            scorecard_number: 1,
            station_number: Some(1),
            competition_name: "Comp",
            event: WcaEvent::E333,
            round_number: 1,
            group_number: 1,
            stage_name: Some("Red Stage"),
            competitor_name: "Alice",
            registrant_id: Some(1),
            wca_id: None,
            attempt_count: 5,
            time_limit_info: None,
            is_blank: false,
            is_cover_sheet: false,
            total_group_cards: 0,
        };

        let opts_cover_on = ResolvedOptions {
            cover_sheets: true,
            cover_sheets_by: vec![CoverSheetBy::Stage],
            ..Default::default()
        };
        let opts_cover_off = ResolvedOptions {
            cover_sheets: false,
            ..Default::default()
        };

        let partitions_ok = vec![("file1.pdf".to_string(), vec![card1])];
        assert!(validate_split_compatibility(&partitions_ok, &opts_cover_on).is_ok());
        assert!(validate_split_compatibility(&partitions_ok, &opts_cover_off).is_ok());

        // Split same bundle across two files
        let partitions_split = vec![
            ("file1.pdf".to_string(), vec![card1]),
            ("file2.pdf".to_string(), vec![card1]),
        ];
        assert!(validate_split_compatibility(&partitions_split, &opts_cover_on).is_err());
        assert!(validate_split_compatibility(&partitions_split, &opts_cover_off).is_ok());

        // Config compatibility: file split more specific than cover sheet criteria
        let opts_incompatible = ResolvedOptions {
            cover_sheets: true,
            cover_sheets_by: vec![CoverSheetBy::Round],
            split: vec![SplitBy::Stage],
            ..Default::default()
        };
        assert!(validate_split_compatibility(&partitions_ok, &opts_incompatible).is_err());
    }
}
