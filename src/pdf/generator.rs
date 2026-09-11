use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{Error as IoError, Write};
use std::path::PathBuf;

use printpdf::ops::PdfPage;
use printpdf::serialize::PdfSaveOptions;
use printpdf::units::Mm;
use printpdf::{FontId, PdfDocument};
use rayon::prelude::*;

use super::font::FontResolver;
use super::layout::{PageFormat, PageLayout};
use super::renderer::ScorecardRenderer;
use crate::scorecard::ScorecardItem;
use crate::wcif::Competition;

/// Error encountered during PDF generation or file serialization.
#[derive(Debug)]
pub enum PdfGenerationError {
    Lopdf(lopdf::Error),
    Io(IoError),
}

impl Display for PdfGenerationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PdfGenerationError::Lopdf(e) => write!(f, "PDF serialization error::Error: {e}"),
            PdfGenerationError::Io(e) => write!(f, "PDF I/O error::Error: {e}"),
        }
    }
}

impl Error for PdfGenerationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            PdfGenerationError::Lopdf(e) => Some(e),
            PdfGenerationError::Io(e) => Some(e),
        }
    }
}

impl From<lopdf::Error> for PdfGenerationError {
    fn from(err: lopdf::Error) -> Self {
        PdfGenerationError::Lopdf(err)
    }
}

impl From<IoError> for PdfGenerationError {
    fn from(err: IoError) -> Self {
        PdfGenerationError::Io(err)
    }
}

/// Generator responsible for parallel page chunking, scorecard rendering, and final PDF byte serialization.
pub struct PdfGenerator {
    pub layout: PageLayout,
    pub format: PageFormat,
    pub start_group_on_new_page: bool,
    pub font_path: Option<PathBuf>,
}

impl PdfGenerator {
    /// Creates a new PdfGenerator with the specified layout and default group format.
    #[cfg(test)]
    pub fn new(layout: PageLayout) -> Self {
        Self {
            layout,
            format: PageFormat::Group,
            start_group_on_new_page: false,
            font_path: None,
        }
    }

    /// Creates a new `PdfGenerator` with specified layout and format.
    #[cfg(test)]
    pub fn with_format(layout: PageLayout, format: PageFormat) -> Self {
        Self::with_options(layout, format, false, None)
    }

    /// Creates a new `PdfGenerator` with specified layout, format, `start_group_on_new_page` flag, and optional custom font path.
    pub fn with_options(
        layout: PageLayout,
        format: PageFormat,
        start_group_on_new_page: bool,
        font_path: Option<PathBuf>,
    ) -> Self {
        Self {
            layout,
            format,
            start_group_on_new_page,
            font_path,
        }
    }

    /// Generates a PDF containing all scorecards and streams directly to any Write destination (e.g. `BufWriter`<File>).
    /// Returns the number of pages generated.
    pub fn generate_to_writer<W: Write>(
        &self,
        comp: &Competition,
        cards: &[ScorecardItem<'_>],
        writer: &mut W,
    ) -> Result<usize, PdfGenerationError> {
        let mut doc = PdfDocument::new(&comp.name);
        let custom_font = FontResolver::resolve(self.font_path.as_deref());
        let font_id = custom_font.as_ref().map(|f| doc.add_font(f));
        let pages = self.build_pages(cards, font_id.as_ref());
        let page_count = pages.len();
        doc.pages = pages;
        Self::save_document_to_writer(&doc, writer)?;
        Ok(page_count)
    }

    /// Builds all PDF pages using parallel Rayon chunk processing.
    pub fn build_pages(
        &self,
        cards: &[ScorecardItem<'_>],
        font_id: Option<&FontId>,
    ) -> Vec<PdfPage> {
        if self.format == PageFormat::Stacked && self.layout.cards_per_page > 1 {
            self.build_stacked_pages(cards, font_id)
        } else {
            self.build_grouped_pages(cards, font_id)
        }
    }

    /// Generates pages in stacked cutting order (card N on page P is followed by N+1 on page P at the same slot).
    fn build_stacked_pages(
        &self,
        cards: &[ScorecardItem<'_>],
        font_id: Option<&FontId>,
    ) -> Vec<PdfPage> {
        let layout = self.layout;
        let total_cards = cards.len();
        let k = layout.cards_per_page;
        let total_pages = total_cards.div_ceil(k);

        (0..total_pages)
            .into_par_iter()
            .map(|page_idx| {
                let mut ops = Vec::with_capacity(256);
                for slot in 0..k {
                    let card_idx = slot * total_pages + page_idx;
                    if card_idx < total_cards {
                        let card = &cards[card_idx];
                        let rect = layout.card_rect(slot);
                        ScorecardRenderer::draw_card(&mut ops, card, rect, font_id);
                    }
                }
                PdfPage::new(Mm(layout.page_w_mm), Mm(layout.page_h_mm), ops)
            })
            .collect()
    }

    /// Generates pages sequentially chunked by `cards_per_page`.
    fn build_grouped_pages(
        &self,
        cards: &[ScorecardItem<'_>],
        font_id: Option<&FontId>,
    ) -> Vec<PdfPage> {
        let layout = self.layout;
        let padded_cards = (self.start_group_on_new_page && layout.cards_per_page > 1)
            .then(|| Self::pad_groups_to_page_boundaries(cards, layout.cards_per_page));
        let effective_cards = padded_cards.as_deref().unwrap_or(cards);

        effective_cards
            .par_chunks(layout.cards_per_page)
            .map(|chunk| {
                let mut ops = Vec::with_capacity(if layout.cards_per_page == 1 { 64 } else { 256 });

                for (idx, card) in chunk.iter().enumerate() {
                    let rect = layout.card_rect(idx);
                    ScorecardRenderer::draw_card(&mut ops, card, rect, font_id);
                }

                PdfPage::new(Mm(layout.page_w_mm), Mm(layout.page_h_mm), ops)
            })
            .collect()
    }

    /// Inserts blank spaces between groups so that the first scorecard/cover sheet
    /// of each group is always on slot 0 (the top-left) of a new page.
    pub fn pad_groups_to_page_boundaries<'a>(
        cards: &[ScorecardItem<'a>],
        cards_per_page: usize,
    ) -> Vec<ScorecardItem<'a>> {
        if cards.is_empty() {
            return Vec::new();
        }
        if cards_per_page <= 1 {
            return cards.to_vec();
        }

        let mut result = Vec::with_capacity(cards.len() + 16);
        let mut current_group = None;
        let mut cards_on_page = 0;

        for &card in cards {
            let group_key = match card {
                ScorecardItem::Scorecard(sc) => {
                    (sc.event, sc.round_number, sc.group_number, sc.stage_name)
                }
                ScorecardItem::Blank(b) => (b.event, b.round_number, b.group_number, b.stage_name),
                ScorecardItem::CoverSheet(c) => {
                    (c.event, c.round_number, c.group_number, c.stage_name)
                }
                ScorecardItem::Empty => continue,
            };

            let is_new_group = current_group.is_some_and(|prev| prev != group_key);
            if (is_new_group || card.is_cover_sheet()) && cards_on_page != 0 {
                let blanks_needed = cards_per_page - cards_on_page;
                for _ in 0..blanks_needed {
                    result.push(ScorecardItem::empty_space());
                }
                cards_on_page = 0;
            }

            current_group = Some(group_key);
            result.push(card);
            cards_on_page = (cards_on_page + 1) % cards_per_page;
        }

        result
    }

    /// Serializes the document to the writer, compressing page streams for compact PDF output.
    fn save_document_to_writer<W: Write>(
        doc: &PdfDocument,
        writer: &mut W,
    ) -> Result<(), PdfGenerationError> {
        let save_options = PdfSaveOptions::default();
        let mut warnings = Vec::new();
        let mut lopdf_doc = doc.to_lopdf_document(&save_options, &mut warnings);
        lopdf_doc.compress();
        lopdf_doc.save_to(writer)?;
        Ok(())
    }

    /// Generates a PDF containing all scorecards into a byte buffer.
    #[cfg(test)]
    pub fn generate(
        &self,
        comp: &Competition,
        cards: &[ScorecardItem<'_>],
    ) -> Result<Vec<u8>, PdfGenerationError> {
        let mut buffer = Vec::new();
        self.generate_to_writer(comp, cards, &mut buffer)?;
        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::layout::PaperSize;
    use crate::scorecard::{Scorecard, WcaEvent};

    #[test]
    fn test_pad_groups_to_page_boundaries_uneven_groups() {
        let group1_card = Scorecard::new(
            "Test Comp",
            WcaEvent::E333,
            1,
            1,
            crate::scorecard::Competitor::simple("Alice"),
        )
        .into();
        let group2_card = Scorecard::new(
            "Test Comp",
            WcaEvent::E333,
            1,
            2,
            crate::scorecard::Competitor::simple("Alice"),
        )
        .into();

        // Group 1 has 3 cards, Group 2 has 2 cards.
        let cards = vec![
            group1_card,
            group1_card,
            group1_card,
            group2_card,
            group2_card,
        ];

        let padded = PdfGenerator::pad_groups_to_page_boundaries(&cards, 4);
        // Expect: 3 cards of G1, 1 blank space, 2 cards of G2 -> 6 items total
        assert_eq!(padded.len(), 6);
        assert!(!padded[0].is_empty());
        assert!(!padded[1].is_empty());
        assert!(!padded[2].is_empty());
        assert!(padded[3].is_empty()); // padding slot 3 on page 1
        if let ScorecardItem::Scorecard(sc) = &padded[4] {
            assert_eq!(sc.group_number, 2); // slot 0 on page 2 (top-left)
        } else {
            panic!("expected Scorecard");
        }
        assert!(!padded[4].is_empty());
        if let ScorecardItem::Scorecard(sc) = &padded[5] {
            assert_eq!(sc.group_number, 2);
        } else {
            panic!("expected Scorecard");
        }
    }

    #[test]
    fn test_pad_groups_to_page_boundaries_exact_multiple() {
        let g1 = Scorecard::new(
            "Test Comp",
            WcaEvent::E333,
            1,
            1,
            crate::scorecard::Competitor::simple("Alice"),
        )
        .into();
        let g2 = Scorecard::new(
            "Test Comp",
            WcaEvent::E333,
            1,
            2,
            crate::scorecard::Competitor::simple("Alice"),
        )
        .into();

        // Group 1 has exactly 4 cards (1 full page)
        let cards = vec![g1, g1, g1, g1, g2];

        let padded = PdfGenerator::pad_groups_to_page_boundaries(&cards, 4);
        // Zero blank spaces needed
        assert_eq!(padded.len(), 5);
        assert!(!padded.iter().any(ScorecardItem::is_empty));
    }

    #[test]
    fn test_pad_groups_to_page_boundaries_single_card_per_page() {
        let g1 = Scorecard::new(
            "Test Comp",
            WcaEvent::E333,
            1,
            1,
            crate::scorecard::Competitor::simple("Alice"),
        )
        .into();
        let g2 = Scorecard::new(
            "Test Comp",
            WcaEvent::E333,
            1,
            2,
            crate::scorecard::Competitor::simple("Alice"),
        )
        .into();

        let cards = vec![g1, g2];
        let padded = PdfGenerator::pad_groups_to_page_boundaries(&cards, 1);
        assert_eq!(padded.len(), 2);
        assert!(!padded.iter().any(ScorecardItem::is_empty));
    }

    #[test]
    fn test_generator_with_start_group_on_new_page() {
        let layout = PageLayout::new(PaperSize::A4);
        let pdf_gen = PdfGenerator::with_options(layout, PageFormat::Group, true, None);
        assert!(pdf_gen.start_group_on_new_page);

        let g1 = Scorecard::new(
            "Test Comp",
            WcaEvent::E333,
            1,
            1,
            crate::scorecard::Competitor::simple("Alice"),
        )
        .into();
        let g2 = Scorecard::new(
            "Test Comp",
            WcaEvent::E333,
            1,
            2,
            crate::scorecard::Competitor::simple("Alice"),
        )
        .into();

        // G1 has 2 cards, G2 has 2 cards. On 4-card layout without padding = 1 page.
        // With start_group_on_new_page = 2 pages (G1 on page 1 with 2 blanks, G2 on page 2).
        let cards = vec![g1, g1, g2, g2];
        let pages = pdf_gen.build_pages(&cards, None);
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn test_brampton_summer_pages() {
        use std::path::Path;

        use crate::options::CoverSheetBy;
        use crate::scorecard::{PlanConfig, ScorecardPlanner};
        use crate::wcif::loader::WcifLoader;

        let Ok(comp) = WcifLoader::load_from_file(Path::new("BramptonSummer.json")) else {
            return;
        };

        // Test with cover sheets = stage
        let plan = ScorecardPlanner::plan(
            &comp,
            &["333-r1"],
            PlanConfig::new(true, &[CoverSheetBy::Stage], true, false, false),
        )
        .unwrap();
        let layout = PageLayout::new(PaperSize::A4);
        let padded = PdfGenerator::pad_groups_to_page_boundaries(&plan, layout.cards_per_page);

        // Helper to access page p (1-indexed, 4 cards per page)
        let page = |p: usize| &padded[(p - 1) * 4..p * 4];

        // Verify that Page 5 (last page of G1 Blue Stage) has 1 scorecard and 3 empty padding items:
        let page5 = page(5);
        assert!(matches!(page5[0], ScorecardItem::Scorecard(_)));
        assert!(matches!(page5[1], ScorecardItem::Empty));
        assert!(matches!(page5[2], ScorecardItem::Empty));
        assert!(matches!(page5[3], ScorecardItem::Empty));

        // Verify Page 6 starts with the Red Stage cover sheet on slot 0:
        if let ScorecardItem::CoverSheet(cs) = page(6)[0] {
            assert_eq!(cs.group_number, 1);
            assert_eq!(cs.stage_name, Some("Red Stage"));
        } else {
            panic!("Expected CoverSheet on Page 6 Slot 0");
        }

        // Verify Page 10 ends G1 Red Stage with 3 empty padding items:
        let page10 = page(10);
        assert!(matches!(page10[0], ScorecardItem::Scorecard(_)));
        assert!(matches!(page10[1], ScorecardItem::Empty));
        assert!(matches!(page10[2], ScorecardItem::Empty));
        assert!(matches!(page10[3], ScorecardItem::Empty));

        // Verify Page 11 starts G2 Blue Stage cover sheet on slot 0:
        if let ScorecardItem::CoverSheet(cs) = page(11)[0] {
            assert_eq!(cs.group_number, 2);
            assert_eq!(cs.stage_name, Some("Blue Stage"));
        } else {
            panic!("Expected CoverSheet on Page 11 Slot 0");
        }
    }
}
