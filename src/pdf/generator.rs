use super::layout::{PageFormat, PageLayout};
use super::renderer::ScorecardRenderer;
use crate::scorecard::ScorecardItem;
use crate::wcif::Competition;
use printpdf::PdfDocument;
use printpdf::ops::PdfPage;
use printpdf::serialize::PdfSaveOptions;
use printpdf::units::Mm;
use rayon::prelude::*;
use std::io::Write;

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
}

impl PdfGenerator {
    /// Creates a new PdfGenerator with the specified layout and default group format.
    #[cfg(test)]
    pub fn new(layout: PageLayout) -> Self {
        Self {
            layout,
            format: PageFormat::Group,
        }
    }

    /// Creates a new PdfGenerator with specified layout and format.
    pub fn with_format(layout: PageLayout, format: PageFormat) -> Self {
        Self { layout, format }
    }

    /// Generates a PDF containing all scorecards and streams directly to any Write destination (e.g. BufWriter<File>).
    /// Returns the number of pages generated.
    pub fn generate_to_writer<W: Write>(
        &self,
        comp: &Competition,
        cards: &[ScorecardItem<'_>],
        writer: &mut W,
    ) -> Result<usize, PdfGenerationError> {
        let pages = self.build_pages(cards);
        let page_count = pages.len();
        let doc = Self::build_pdf_document(&comp.name, pages);
        Self::save_document_to_writer(&doc, writer)?;
        Ok(page_count)
    }

    /// Builds all PDF pages using parallel Rayon chunk processing.
    pub fn build_pages(&self, cards: &[ScorecardItem<'_>]) -> Vec<PdfPage> {
        if self.format == PageFormat::Stacked && self.layout.cards_per_page > 1 {
            self.build_stacked_pages(cards)
        } else {
            self.build_grouped_pages(cards)
        }
    }

    /// Generates pages in stacked cutting order (card N on page P is followed by N+1 on page P at the same slot).
    fn build_stacked_pages(&self, cards: &[ScorecardItem<'_>]) -> Vec<PdfPage> {
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
                        ScorecardRenderer::draw_card(&mut ops, card, rect);
                    }
                }
                PdfPage::new(Mm(layout.page_w_mm), Mm(layout.page_h_mm), ops)
            })
            .collect()
    }

    /// Generates pages sequentially chunked by cards_per_page.
    fn build_grouped_pages(&self, cards: &[ScorecardItem<'_>]) -> Vec<PdfPage> {
        let layout = self.layout;
        cards
            .par_chunks(layout.cards_per_page)
            .map(|chunk| {
                let mut ops = Vec::with_capacity(if layout.cards_per_page == 1 { 64 } else { 256 });

                for (idx, card) in chunk.iter().enumerate() {
                    let rect = layout.card_rect(idx);
                    ScorecardRenderer::draw_card(&mut ops, card, rect);
                }

                PdfPage::new(Mm(layout.page_w_mm), Mm(layout.page_h_mm), ops)
            })
            .collect()
    }

    /// Assembles a PdfDocument model from a list of generated PdfPages.
    fn build_pdf_document(title: &str, pages: Vec<PdfPage>) -> PdfDocument {
        let mut doc = PdfDocument::new(title);
        doc.pages = pages;
        doc
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
