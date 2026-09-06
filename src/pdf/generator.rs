use super::font::FontResolver;
use super::layout::{PageFormat, PageLayout};
use super::renderer::ScorecardRenderer;
use crate::scorecard::ScorecardItem;
use crate::wcif::Competition;
use printpdf::ops::PdfPage;
use printpdf::serialize::PdfSaveOptions;
use printpdf::units::Mm;
use printpdf::{FontId, PdfDocument};
use rayon::prelude::*;
use std::io::Write;
use std::path::PathBuf;

/// Error encountered during PDF generation or file serialization.
#[derive(Debug)]
pub enum PdfGenerationError {
    Lopdf(lopdf::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for PdfGenerationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfGenerationError::Lopdf(e) => write!(f, "PDF serialization error: {e}"),
            PdfGenerationError::Io(e) => write!(f, "PDF I/O error: {e}"),
        }
    }
}

impl std::error::Error for PdfGenerationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
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

impl From<std::io::Error> for PdfGenerationError {
    fn from(err: std::io::Error) -> Self {
        PdfGenerationError::Io(err)
    }
}

/// Generator responsible for parallel page chunking, scorecard rendering, and final PDF byte serialization.
pub struct PdfGenerator {
    pub layout: PageLayout,
    pub format: PageFormat,
    pub font_path: Option<PathBuf>,
}

impl PdfGenerator {
    /// Creates a new PdfGenerator with the specified layout and default group format.
    #[cfg(test)]
    pub fn new(layout: PageLayout) -> Self {
        Self {
            layout,
            format: PageFormat::Group,
            font_path: None,
        }
    }

    /// Creates a new `PdfGenerator` with specified layout and format.
    #[cfg(test)]
    pub fn with_format(layout: PageLayout, format: PageFormat) -> Self {
        Self::with_font_path(layout, format, None)
    }

    /// Creates a new `PdfGenerator` with specified layout, format, and optional custom font path.
    pub fn with_font_path(
        layout: PageLayout,
        format: PageFormat,
        font_path: Option<PathBuf>,
    ) -> Self {
        Self {
            layout,
            format,
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
        cards
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
