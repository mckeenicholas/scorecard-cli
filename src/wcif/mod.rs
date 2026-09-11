pub mod advancement;
pub mod loader;
pub mod model;

pub use advancement::AdvancementCalculator;
pub use loader::{WcifLoadError, WcifLoader, expand_tilde};
pub use model::{
    Competition, Cutoff, Event, GroupifierCompetitionConfig, Person, Round, ScheduledActivityInfo,
    TimeLimit, WcaId,
};

#[cfg(test)]
mod tests {
    use std::path::Path;

    use model::CountryIso2;

    use super::*;

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
        assert_eq!(comp.persons[0].country_iso2, CountryIso2::parse("US"));
        assert_eq!(comp.persons[1].country_iso2, CountryIso2::parse("CA"));
    }

    #[test]
    fn test_country_iso2() {
        let code = CountryIso2::parse("CA").expect("valid country code");
        assert_eq!(code.as_str(), "CA");
        assert_eq!(&*code, "CA");
        assert_eq!(code, "CA");
        assert_eq!(code.to_bytes(), *b"CA");
        assert_eq!(code.0, *b"CA");
        assert_eq!(format!("{code}"), "CA");

        assert!(CountryIso2::parse("C").is_none());
        assert!(CountryIso2::parse("CAN").is_none());
        assert!(CountryIso2::parse("").is_none());

        // Serde roundtrip
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"CA\"");
        let deserialized: CountryIso2 = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, code);

        // Streaming deserialization via io::Read
        let reader = std::io::Cursor::new(b"\"CA\"");
        let deserialized_stream: CountryIso2 = serde_json::from_reader(reader).unwrap();
        assert_eq!(deserialized_stream, code);
    }

    #[test]
    fn test_wca_id_and_country_iso2_streaming_deserialization() {
        use std::io::Cursor;

        // Person with full WCA ID and country via reader stream
        let json =
            r#"{"name":"Test User","wcaId":"2010AMBR01","countryIso2":"US","registrantId":1}"#;
        let p: Person = serde_json::from_reader(Cursor::new(json.as_bytes())).unwrap();
        assert_eq!(p.name, "Test User");
        assert_eq!(p.wca_id, WcaId::parse("2010AMBR01"));
        assert_eq!(p.country_iso2, CountryIso2::parse("US"));

        // Person with empty string wcaId and countryIso2 via reader stream
        let json_empty = r#"{"name":"Newcomer","wcaId":"","countryIso2":"","registrantId":2}"#;
        let p_empty: Person = serde_json::from_reader(Cursor::new(json_empty.as_bytes())).unwrap();
        assert_eq!(p_empty.name, "Newcomer");
        assert_eq!(p_empty.wca_id, None);
        assert_eq!(p_empty.country_iso2, None);

        // Person with null wcaId and countryIso2 via reader stream
        let json_null = r#"{"name":"Null Guy","wcaId":null,"countryIso2":null,"registrantId":3}"#;
        let p_null: Person = serde_json::from_reader(Cursor::new(json_null.as_bytes())).unwrap();
        assert_eq!(p_null.name, "Null Guy");
        assert_eq!(p_null.wca_id, None);
        assert_eq!(p_null.country_iso2, None);

        // Whole competition via reader stream
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test_wcif.json");
        let file = std::fs::File::open(fixture_path).unwrap();
        let comp: Competition = serde_json::from_reader(file).unwrap();
        assert_eq!(comp.id, "FastCubingSpring2026");
        assert_eq!(comp.persons.len(), 3);
    }

    #[test]
    fn test_round_attempt_counts() {
        let make_round = |fmt: &str| Round {
            id: "333-r1".to_owned(),
            format: Some(fmt.to_owned()),
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
