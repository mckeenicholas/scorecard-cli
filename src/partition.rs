use crate::options::{self, ResolvedOptions, SplitBy};
use crate::pdf::{PageLayout, PdfGenerator};
use crate::progress;
use crate::scorecard::{ScorecardItem, WcaEvent};
use crate::wcif;
use crossterm::style::Stylize;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::BufWriter;

const BUFFER_SIZE: usize = 16 * 1024 * 1024; // 16 MB

pub fn slugify(s: &str) -> String {
    let mut slug = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if (c.is_whitespace() || c == '-' || c == '_')
            && !slug.ends_with('-')
            && !slug.is_empty()
        {
            slug.push('-');
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SplitKey {
    pub stage: Option<String>,
    pub event: Option<(WcaEvent, usize)>,
    pub group: Option<usize>,
}

impl SplitKey {
    pub fn to_filename(&self, comp_id: &str) -> String {
        let mut name = String::with_capacity(comp_id.len() + 48);
        let _ = write!(name, "{comp_id}-scorecards");
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

pub fn build_split_key(
    card: &ScorecardItem<'_>,
    has_stage: bool,
    has_event: bool,
    has_group: bool,
) -> SplitKey {
    let (stage_name, event, round_number, group_number) = match card {
        ScorecardItem::Scorecard(sc) => (
            sc.stage_name,
            Some(sc.event),
            sc.round_number,
            sc.group_number,
        ),
        ScorecardItem::Blank(b) => (b.stage_name, Some(b.event), b.round_number, b.group_number),
        ScorecardItem::CoverSheet(cs) => (
            cs.stage_name,
            Some(cs.event),
            cs.round_number,
            cs.group_number,
        ),
        ScorecardItem::Empty => (None, None, 0, 0),
    };

    SplitKey {
        stage: if has_stage {
            Some(slugify(stage_name.unwrap_or("no-stage")))
        } else {
            None
        },
        event: if has_event && let Some(ev) = event {
            Some((ev, round_number))
        } else {
            None
        },
        group: if has_group { Some(group_number) } else { None },
    }
}

pub fn partition_scorecards<'a>(
    comp_id: &str,
    cards: &'a [ScorecardItem<'a>],
    split_by: &[SplitBy],
) -> Vec<(String, Cow<'a, [ScorecardItem<'a>]>)> {
    if split_by.is_empty() {
        let out_filename = format!("{comp_id}-scorecards.pdf");
        return vec![(out_filename, Cow::Borrowed(cards))];
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
        .map(|(k, v)| (k.to_filename(comp_id), Cow::Owned(v)))
        .collect()
}

pub fn write_pdf_file(
    generator: &PdfGenerator,
    comp: &wcif::Competition,
    filename: &str,
    cards: &[ScorecardItem<'_>],
) -> Result<usize, crate::AppError> {
    let file = File::create(filename).map_err(|source| crate::AppError::CreatePdf {
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
            return Err(crate::AppError::GeneratePdf {
                path: filename.to_string(),
                source,
            });
        }
    };
    spinner.finish_and_clear();
    Ok(page_count)
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

pub fn validate_card_bundle_placement(
    partitions: &[(String, Cow<'_, [ScorecardItem<'_>]>)],
) -> Result<(), SplitError> {
    let mut bundle_partition_map: std::collections::HashMap<BundleKey<'_>, &str> =
        std::collections::HashMap::new();

    for (filename, partition_cards) in partitions {
        for card in partition_cards.as_ref() {
            let (stage_name, event, round_number, group_number) = match card {
                ScorecardItem::Scorecard(sc) => {
                    (sc.stage_name, sc.event, sc.round_number, sc.group_number)
                }
                ScorecardItem::Blank(b) => (b.stage_name, b.event, b.round_number, b.group_number),
                ScorecardItem::CoverSheet(_) | ScorecardItem::Empty => continue,
            };
            let bundle_key = BundleKey {
                stage_name,
                event,
                round_number,
                group_number,
            };
            if let Some(existing_file) = bundle_partition_map.get(&bundle_key) {
                if *existing_file != filename.as_str() {
                    return Err(SplitError::SplitBundle {
                        event_id: event.code().to_string(),
                        round_number,
                        group_number,
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

pub fn validate_split_compatibility(
    partitions: &[(String, Cow<'_, [ScorecardItem<'_>]>)],
    options: &ResolvedOptions,
) -> Result<(), SplitError> {
    options.validate_compatibility()?;

    if options.cover_sheets {
        validate_card_bundle_placement(partitions)?;
    }

    Ok(())
}

pub fn write_and_report_partition(
    generator: &PdfGenerator,
    comp: &wcif::Competition,
    out_filename: &str,
    partition_cards: &[ScorecardItem<'_>],
    is_multi: bool,
) -> Result<usize, crate::AppError> {
    let page_count = write_pdf_file(generator, comp, out_filename, partition_cards)?;
    let icon = "✔".green();
    let real_card_count = partition_cards.iter().filter(|c| !c.is_empty()).count();
    let scorecards_word = if real_card_count == 1 {
        "scorecard"
    } else {
        "scorecards"
    };
    let pages_word = if page_count == 1 { "page" } else { "pages" };
    if is_multi {
        println!(
            "  {icon} Generated {} ({} {}, {} {})",
            out_filename.cyan(),
            real_card_count,
            scorecards_word,
            page_count,
            pages_word
        );
    } else {
        println!(
            "{icon} Successfully generated scorecards PDF: {} ({} {}, {} {})",
            out_filename.cyan(),
            real_card_count,
            scorecards_word,
            page_count,
            pages_word
        );
    }
    Ok(page_count)
}

pub fn generate_partitioned_pdfs(
    comp: &wcif::Competition,
    cards: &[ScorecardItem<'_>],
    options: &ResolvedOptions,
) -> Result<(), crate::AppError> {
    let layout = PageLayout::new(options.paper);
    let generator = PdfGenerator::with_options(
        layout,
        options.format,
        options.start_group_on_new_page,
        options.font.clone(),
    );
    let partitions = partition_scorecards(&comp.id, cards, &options.split);
    validate_split_compatibility(&partitions, options)?;
    let is_multi = partitions.len() > 1;

    if is_multi {
        println!("Generating {} separate PDF files...", partitions.len());
    }

    let mut total_pages = 0;
    for (out_filename, partition_cards) in &partitions {
        let pages = write_and_report_partition(
            &generator,
            comp,
            out_filename,
            partition_cards.as_ref(),
            is_multi,
        )?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scorecard::Competitor;
    use std::num::NonZeroUsize;

    const ID1: NonZeroUsize = NonZeroUsize::MIN;
    const ID2: NonZeroUsize = match NonZeroUsize::new(2) {
        Some(n) => n,
        None => unreachable!(),
    };

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
            ScorecardItem::scorecard(
                "Comp",
                WcaEvent::E333,
                1,
                1,
                Some("Red Stage"),
                Competitor {
                    name: "Alice",
                    local_name: None,
                    registrant_id: ID1,
                    wca_id: None,
                },
                Some(1),
                5,
                None,
            ),
            ScorecardItem::scorecard(
                "Comp",
                WcaEvent::E222,
                1,
                2,
                Some("Blue Stage"),
                Competitor {
                    name: "Bob",
                    local_name: None,
                    registrant_id: ID2,
                    wca_id: None,
                },
                Some(2),
                5,
                None,
            ),
        ];

        let partitions = partition_scorecards("Comp2026", &cards, &[]);
        assert_eq!(partitions.len(), 1);
        assert_eq!(partitions[0].0, "Comp2026-scorecards.pdf");
        assert_eq!(partitions[0].1.len(), 2);
    }

    #[test]
    fn test_partition_scorecards_by_event() {
        let cards = vec![
            ScorecardItem::scorecard(
                "Comp",
                WcaEvent::E333,
                1,
                1,
                Some("Red Stage"),
                Competitor {
                    name: "Alice",
                    local_name: None,
                    registrant_id: ID1,
                    wca_id: None,
                },
                Some(1),
                5,
                None,
            ),
            ScorecardItem::scorecard(
                "Comp",
                WcaEvent::E222,
                1,
                2,
                Some("Blue Stage"),
                Competitor {
                    name: "Bob",
                    local_name: None,
                    registrant_id: ID2,
                    wca_id: None,
                },
                Some(2),
                5,
                None,
            ),
        ];

        let partitions = partition_scorecards("Comp2026", &cards, &[SplitBy::Event]);
        assert_eq!(partitions.len(), 2);
        assert_eq!(partitions[0].0, "Comp2026-scorecards-333-r1.pdf");
        assert_eq!(partitions[1].0, "Comp2026-scorecards-222-r1.pdf");
    }

    #[test]
    fn test_partition_scorecards_by_all_three() {
        let cards = vec![
            ScorecardItem::scorecard(
                "Comp",
                WcaEvent::E333,
                1,
                1,
                Some("Red Stage"),
                Competitor {
                    name: "Alice",
                    local_name: None,
                    registrant_id: ID1,
                    wca_id: None,
                },
                Some(1),
                5,
                None,
            ),
            ScorecardItem::scorecard(
                "Comp",
                WcaEvent::E333,
                1,
                2,
                Some("Red Stage"),
                Competitor {
                    name: "Bob",
                    local_name: None,
                    registrant_id: ID2,
                    wca_id: None,
                },
                Some(2),
                5,
                None,
            ),
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
        let card1 = ScorecardItem::scorecard(
            "Comp",
            WcaEvent::E333,
            1,
            1,
            Some("Red Stage"),
            Competitor {
                name: "Alice",
                local_name: None,
                registrant_id: ID1,
                wca_id: None,
            },
            Some(1),
            5,
            None,
        );

        let opts_cover_on = ResolvedOptions {
            cover_sheets: true,
            cover_sheets_by: vec![CoverSheetBy::Stage],
            ..Default::default()
        };
        let opts_cover_off = ResolvedOptions {
            cover_sheets: false,
            ..Default::default()
        };

        let partitions_ok = vec![(
            "file1.pdf".to_string(),
            Cow::Borrowed(std::slice::from_ref(&card1)),
        )];
        assert!(validate_split_compatibility(&partitions_ok, &opts_cover_on).is_ok());
        assert!(validate_split_compatibility(&partitions_ok, &opts_cover_off).is_ok());

        // Split same bundle across two files
        let partitions_split = vec![
            (
                "file1.pdf".to_string(),
                Cow::Borrowed(std::slice::from_ref(&card1)),
            ),
            (
                "file2.pdf".to_string(),
                Cow::Borrowed(std::slice::from_ref(&card1)),
            ),
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
