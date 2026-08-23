mod options;
mod pdf;
mod progress;
mod scorecard;
mod wcif;

use clap::Parser;
use mimalloc::MiMalloc;
use options::{Cli, ResolvedOptions, ShardBy};
use pdf::{PageLayout, PdfGenerator};
use scorecard::{ScorecardItem, ScorecardPlanner};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Write as _;
use std::fs::File;
use std::io::BufWriter;
use std::process;
use wcif::WcifLoader;

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
struct ShardKey {
    stage: Option<String>,
    event: Option<String>,
    group: Option<usize>,
}

impl ShardKey {
    fn to_filename(&self, comp_id: &str) -> String {
        let mut name = format!("{}-scorecards", comp_id);
        if let Some(ref stage) = self.stage {
            name.push('-');
            name.push_str(stage);
        }
        if let Some(ref event) = self.event {
            name.push('-');
            name.push_str(event);
        }
        if let Some(group) = self.group {
            write!(name, "-group{}", group).unwrap();
        }
        name.push_str(".pdf");
        name
    }
}

fn build_shard_key(
    card: &ScorecardItem<'_>,
    has_stage: bool,
    has_event: bool,
    has_group: bool,
) -> ShardKey {
    ShardKey {
        stage: if has_stage {
            Some(slugify(card.stage_name.unwrap_or("no-stage")))
        } else {
            None
        },
        event: if has_event {
            Some(format!("{}-r{}", card.event_id, card.round_number))
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
    shard_by: &[ShardBy],
) -> Vec<(String, Vec<ScorecardItem<'a>>)> {
    if shard_by.is_empty() {
        let out_filename = format!("{}-scorecards.pdf", comp_id);
        return vec![(out_filename, cards.to_vec())];
    }

    let has_stage = shard_by.contains(&ShardBy::Stage);
    let has_event = shard_by.contains(&ShardBy::Event);
    let has_group = shard_by.contains(&ShardBy::Group);

    let mut map: BTreeMap<ShardKey, Vec<ScorecardItem<'a>>> = BTreeMap::new();

    for card in cards {
        let key = build_shard_key(card, has_stage, has_event, has_group);
        map.entry(key).or_default().push(*card);
    }

    map.into_iter()
        .map(|(k, v)| (k.to_filename(comp_id), v))
        .collect()
}

fn write_pdf_file(
    generator: &PdfGenerator,
    comp: &wcif::Competition,
    filename: &str,
    cards: &[ScorecardItem<'_>],
) -> Result<(), Box<dyn Error>> {
    let file = File::create(filename)
        .map_err(|e| format!("Error creating output PDF file {}: {}", filename, e))?;
    let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);

    let spinner = progress::create_spinner(format!(
        "Generating {} scorecards to PDF ({})...",
        cards.len(),
        filename
    ));

    if let Err(err) = generator.generate_to_writer(comp, cards, &mut writer) {
        spinner.finish_and_clear();
        return Err(format!("Error generating scorecards PDF: {}", err).into());
    }
    spinner.finish_and_clear();
    Ok(())
}

fn load_competition(source: &str) -> Result<wcif::Competition, Box<dyn Error>> {
    println!("Loading WCIF from: {}...", source);
    let comp = WcifLoader::load(source).map_err(|e| format!("Error loading WCIF: {}", e))?;
    println!("Loaded competition: {} ({})", comp.name, comp.id);
    Ok(comp)
}

fn resolve_options(cli: &Cli, comp: &wcif::Competition) -> ResolvedOptions {
    let groupifier_config = comp.get_groupifier_config();
    let active_opts = ResolvedOptions::resolve(cli, groupifier_config.as_ref());
    active_opts.print_summary();
    active_opts
}

fn validate_sharding_compatibility(
    partitions: &[(String, Vec<ScorecardItem<'_>>)],
    cover_sheets: bool,
) -> Result<(), Box<dyn Error>> {
    if !cover_sheets {
        return Ok(());
    }

    // Verify that all cards belonging to the same group end up in the exact same partition file
    let mut group_partition_map: std::collections::HashMap<
        (String, usize, usize, Option<String>),
        &str,
    > = std::collections::HashMap::new();

    for (filename, partition_cards) in partitions {
        for card in partition_cards {
            let group_key = (
                card.event_id.to_string(),
                card.round_number,
                card.group_number,
                card.stage_name.map(|s| s.to_string()),
            );
            if let Some(existing_file) = group_partition_map.get(&group_key) {
                if *existing_file != filename.as_str() {
                    return Err(format!(
                        "Incompatible sharding: Group {} for event {} round {} is split across files '{}' and '{}'. Cover sheets require each group to remain intact in a single file.",
                        card.group_number, card.event_id, card.round_number, existing_file, filename
                    ).into());
                }
            } else {
                group_partition_map.insert(group_key, filename.as_str());
            }
        }
    }

    Ok(())
}

fn generate_partitioned_pdfs(
    comp: &wcif::Competition,
    cards: &[ScorecardItem<'_>],
    options: &ResolvedOptions,
) -> Result<(), Box<dyn Error>> {
    let layout = PageLayout::new(options.paper);
    let generator = PdfGenerator::with_format(layout, options.format);
    let partitions = partition_scorecards(&comp.id, cards, &options.shard);
    validate_sharding_compatibility(&partitions, options.cover_sheets)?;
    let is_multi = partitions.len() > 1;

    if is_multi {
        println!("\nGenerating {} sharded PDF files...", partitions.len());
    }

    for (out_filename, shard_cards) in &partitions {
        write_pdf_file(&generator, comp, out_filename, shard_cards)?;
        if is_multi {
            println!(
                "  [+] Generated {} ({} cards)",
                out_filename,
                shard_cards.len()
            );
        } else {
            println!("Successfully generated scorecards PDF: {}", out_filename);
        }
    }

    if is_multi {
        println!(
            "\nSuccessfully generated all {} sharded PDFs.",
            partitions.len()
        );
    }

    Ok(())
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let comp = load_competition(&cli.comp_source)?;
    let active_opts = resolve_options(&cli, &comp);
    let plan = ScorecardPlanner::plan(&comp, &cli.events, active_opts.cover_sheets)?;

    if plan.is_empty() {
        println!("No scorecards to generate.");
        return Ok(());
    }

    plan.print_summary();
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
                event_id: "333",
                event_name: "3x3x3 Cube",
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
                event_id: "222",
                event_name: "2x2x2 Cube",
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
                event_id: "333",
                event_name: "3x3x3 Cube",
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
                event_id: "222",
                event_name: "2x2x2 Cube",
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

        let partitions = partition_scorecards("Comp2026", &cards, &[ShardBy::Event]);
        assert_eq!(partitions.len(), 2);
        assert_eq!(partitions[0].0, "Comp2026-scorecards-222-r1.pdf");
        assert_eq!(partitions[1].0, "Comp2026-scorecards-333-r1.pdf");
    }

    #[test]
    fn test_partition_scorecards_by_all_three() {
        let cards = vec![
            ScorecardItem {
                scorecard_number: 1,
                station_number: Some(1),
                competition_name: "Comp",
                event_id: "333",
                event_name: "3x3x3 Cube",
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
                event_id: "333",
                event_name: "3x3x3 Cube",
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
            &[ShardBy::Stage, ShardBy::Event, ShardBy::Group],
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
    fn test_validate_sharding_compatibility() {
        let card1 = ScorecardItem {
            scorecard_number: 1,
            station_number: Some(1),
            competition_name: "Comp",
            event_id: "333",
            event_name: "3x3x3 Cube",
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

        let partitions_ok = vec![("file1.pdf".to_string(), vec![card1])];
        assert!(validate_sharding_compatibility(&partitions_ok, true).is_ok());
        assert!(validate_sharding_compatibility(&partitions_ok, false).is_ok());

        // Split same group across two files
        let partitions_split = vec![
            ("file1.pdf".to_string(), vec![card1]),
            ("file2.pdf".to_string(), vec![card1]),
        ];
        assert!(validate_sharding_compatibility(&partitions_split, true).is_err());
        assert!(validate_sharding_compatibility(&partitions_split, false).is_ok());
    }
}
