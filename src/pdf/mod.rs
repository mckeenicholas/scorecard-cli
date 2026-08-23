pub mod generator;
pub mod layout;
pub mod renderer;

pub use generator::PdfGenerator;
pub use layout::PageLayout;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scorecard::ScorecardItem;
    use crate::wcif::Competition;

    #[test]
    fn test_pdf_generator_formats() {
        let comp = Competition {
            format_version: Some("1.0".to_string()),
            id: "TestComp".to_string(),
            name: "Test Competition 2026".to_string(),
            short_name: Some("Test Comp".to_string()),
            persons: vec![],
            events: vec![],
            schedule: None,
            extensions: vec![],
        };

        let cards = vec![
            ScorecardItem {
                scorecard_number: 1,
                station_number: Some(1),
                competition_name: "Test Comp",
                event_id: "333",
                event_name: "3x3x3 Cube",
                round_number: 1,
                group_number: 1,
                competitor_name: "Alice Smith",
                registrant_id: Some(1),
                wca_id: Some("2022SMIT01"),
                attempt_count: 5,
                is_blank: false,
            },
            ScorecardItem {
                scorecard_number: 2,
                station_number: None,
                competition_name: "Test Comp",
                event_id: "333",
                event_name: "3x3x3 Cube",
                round_number: 2,
                group_number: 1,
                competitor_name: "",
                registrant_id: None,
                wca_id: None,
                attempt_count: 5,
                is_blank: true,
            },
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
    }
}
