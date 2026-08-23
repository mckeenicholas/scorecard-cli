use super::layout::PageLayout;
use super::renderer::ScorecardRenderer;
use crate::scorecard::ScorecardItem;
use crate::wcif::Competition;
use printpdf::PdfDocument;
use printpdf::ops::PdfPage;
use printpdf::serialize::PdfSaveOptions;
use printpdf::units::Mm;
use rayon::prelude::*;
use std::error::Error;
use std::io::Write;

/// Generator responsible for parallel page chunking, scorecard rendering, and final PDF byte serialization.
pub struct PdfGenerator {
    pub layout: PageLayout,
}

impl PdfGenerator {
    /// Creates a new PdfGenerator with the specified layout.
    pub fn new(layout: PageLayout) -> Self {
        Self { layout }
    }

    /// Generates a PDF containing all scorecards and streams directly to any Write destination (e.g. BufWriter<File>).
    pub fn generate_to_writer<W: Write>(
        &self,
        comp: &Competition,
        cards: &[ScorecardItem<'_>],
        writer: &mut W,
    ) -> Result<(), Box<dyn Error>> {
        let layout = self.layout;

        // Parallel chunk processing across CPU cores with Rayon
        let pages: Vec<PdfPage> = cards
            .par_chunks(layout.cards_per_page)
            .map(|chunk| {
                let mut ops = Vec::with_capacity(if layout.cards_per_page == 1 { 64 } else { 256 });

                for (idx, card) in chunk.iter().enumerate() {
                    let rect = layout.card_rect(idx);
                    ScorecardRenderer::draw_card(&mut ops, card, rect.x, rect.y, rect.w, rect.h);
                }

                PdfPage::new(Mm(layout.page_w_mm), Mm(layout.page_h_mm), ops)
            })
            .collect();

        let mut doc = PdfDocument::new(&comp.name);
        doc.pages = pages;

        let save_options = PdfSaveOptions {
            optimize: false, // Disable expensive extra compression passes for maximum speed
            ..Default::default()
        };

        let mut warnings = Vec::new();
        doc.save_writer(writer, &save_options, &mut warnings);

        Ok(())
    }

    /// Generates a PDF containing all scorecards into a byte buffer.
    #[cfg(test)]
    pub fn generate(
        &self,
        comp: &Competition,
        cards: &[ScorecardItem<'_>],
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut buffer = Vec::new();
        self.generate_to_writer(comp, cards, &mut buffer)?;
        Ok(buffer)
    }
}
