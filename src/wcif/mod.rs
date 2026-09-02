pub mod advancement;
pub mod loader;
pub mod model;

pub use advancement::AdvancementCalculator;
pub use loader::{WcifLoader, expand_tilde};
pub use model::{
    Competition, Cutoff, Event, GroupifierCompetitionConfig, Person, Round, ScheduledActivityInfo,
    TimeLimit,
};

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

    #[test]
    fn test_round_attempt_counts() {
        let make_round = |fmt: &str| Round {
            id: "333-r1".to_string(),
            format: Some(fmt.to_string()),
            time_limit: None,
            cutoff: None,
            advancement_condition: None,
            scramble_group_count: 1,
        };

        assert_eq!(make_round("1").attempt_count(), 1);
        assert_eq!(make_round("2").attempt_count(), 2);
        assert_eq!(make_round("3").attempt_count(), 3);
        assert_eq!(make_round("m").attempt_count(), 3);
        assert_eq!(make_round("5").attempt_count(), 5);
        assert_eq!(make_round("a").attempt_count(), 5);
        assert_eq!(make_round("other").attempt_count(), 5);
    }

    #[test]
    fn test_activity_schedule_map() {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test_wcif.json");
        let comp = WcifLoader::load_from_file(&fixture_path).expect("Failed to load test WCIF");

        let activity_map = comp.build_activity_schedule_map();
        // Activity 101 should map to "333-r1-g1"
        let act101 = activity_map.get(&101);
        assert!(act101.is_some());
        assert_eq!(act101.unwrap().activity_code, "333-r1-g1");
        assert_eq!(act101.unwrap().room_name, None);

        // Parent activity 100 should map to "333-r1"
        let act100 = activity_map.get(&100);
        assert!(act100.is_some());
        assert_eq!(act100.unwrap().activity_code, "333-r1");
    }
}
