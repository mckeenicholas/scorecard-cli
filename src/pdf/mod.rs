pub mod attempt;
pub mod font;
pub mod generator;
pub mod layout;
pub mod renderer;
pub mod table;
pub mod text;
pub mod theme;

pub use generator::{PdfGenerationError, PdfGenerator};
pub use layout::{PageFormat, PageLayout, PaperSize};

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::*;
    use crate::scorecard::{
        BlankScorecard, Competitor, Scorecard, TimeLimitInfo, WcaEvent, WcaResult,
    };
    use crate::wcif::{Competition, WcaId};

    #[test]
    fn test_pdf_generator_formats() {
        let comp = Competition {
            format_version: Some("1.0".to_owned()),
            id: "TestComp".to_owned(),
            name: "Test Competition 2026".to_owned(),
            short_name: Some("Test Comp".to_owned()),
            persons: vec![],
            events: vec![],
            schedule: None,
            extensions: vec![],
        };

        let cards = vec![
            Scorecard::new(
                "Test Comp",
                WcaEvent::E333,
                1,
                1,
                Competitor {
                    name: "Alice Smith",
                    local_name: None,
                    registrant_id: NonZeroUsize::MIN,
                    wca_id: WcaId::parse("2022SMIT01"),
                },
            )
            .with_stage(Some("Main Stage"))
            .with_station(Some(1))
            .with_time_limit(Some(TimeLimitInfo {
                limit_centiseconds: WcaResult::new(60000),
                is_cumulative: false,
                cutoff_centiseconds: None,
                cutoff_attempts: 0,
            }))
            .into(),
            BlankScorecard::new("Test Comp", WcaEvent::E333, 2, 1).into(),
        ];

        let gen_a4 = PdfGenerator::new(PageLayout::new(layout::PaperSize::A4));
        let pdf_a4 = gen_a4
            .generate(&comp, &cards)
            .expect("a4 generation failed");
        assert!(!pdf_a4.is_empty());
        assert!(pdf_a4.starts_with(b"%PDF-"));

        let gen_letter = PdfGenerator::new(PageLayout::new(layout::PaperSize::Letter));
        let pdf_letter = gen_letter
            .generate(&comp, &cards)
            .expect("letter generation failed");
        assert!(!pdf_letter.is_empty());
        assert!(pdf_letter.starts_with(b"%PDF-"));

        let gen_a6 = PdfGenerator::new(PageLayout::new(layout::PaperSize::A6));
        let pdf_a6 = gen_a6
            .generate(&comp, &cards)
            .expect("a6 generation failed");
        assert!(!pdf_a6.is_empty());
        assert!(pdf_a6.starts_with(b"%PDF-"));

        let gen_stacked = PdfGenerator::with_format(
            PageLayout::new(layout::PaperSize::A4),
            layout::PageFormat::Stacked,
        );
        let pdf_stacked = gen_stacked
            .generate(&comp, &cards)
            .expect("stacked generation failed");
        assert!(!pdf_stacked.is_empty());
        assert!(pdf_stacked.starts_with(b"%PDF-"));
    }
}
