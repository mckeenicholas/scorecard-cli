pub mod advancement;
pub mod loader;
pub mod model;

pub use advancement::AdvancementCalculator;
pub use loader::WcifLoader;
pub use model::{Competition, Event, GroupifierCompetitionConfig, Round};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_load_wcif_from_file() {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test_wcif.json");
        let comp = WcifLoader::load_from_file(&fixture_path).expect("Failed to load test WCIF");

        assert_eq!(comp.id, "FastCubingSpring2026");
        assert_eq!(comp.name, "Fast Cubing Spring 2026");
        assert_eq!(comp.display_name(), "Fast Spring 2026");
        assert_eq!(comp.persons.len(), 3);
        assert_eq!(comp.events.len(), 2);

        let groupifier = comp.get_groupifier_config();
        assert!(groupifier.is_some());
        let config = groupifier.unwrap();
        assert_eq!(config.scorecard_paper_size.as_deref(), Some("a4"));
        assert_eq!(config.print_scorecards_cover_sheets, Some(true));

        assert_eq!(comp.count_competitors_for_event("333"), 3);
        assert_eq!(comp.count_competitors_for_event("222"), 2);
    }
}
