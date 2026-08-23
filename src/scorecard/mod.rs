pub mod events;
pub mod model;
pub mod planner;

pub use model::ScorecardItem;
pub use planner::ScorecardPlanner;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_name_by_id() {
        assert_eq!(events::event_name_by_id("333").unwrap(), "3x3x3 Cube");
        assert_eq!(events::event_name_by_id("222").unwrap(), "2x2x2 Cube");
        assert_eq!(
            events::event_name_by_id("333bf").unwrap(),
            "3x3x3 Blindfolded"
        );
        assert_eq!(events::event_name_by_id("sq1").unwrap(), "Square-1");
        assert!(events::event_name_by_id("invalid_event").is_err());
        assert_eq!(
            events::event_name_by_id("invalid_event").unwrap_err(),
            events::UnknownEventError("invalid_event".to_string())
        );
    }

    #[test]
    fn test_scorecard_item_display_methods() {
        let item = ScorecardItem {
            scorecard_number: 42,
            station_number: Some(7),
            competition_name: "Very Long Competition Name 2026",
            event_id: "333",
            event_name: "3x3x3 Cube",
            round_number: 1,
            group_number: 1,
            competitor_name: "Alice Smith",
            registrant_id: Some(10),
            wca_id: Some("2022SMIT01"),
            attempt_count: 5,
            is_blank: false,
        };

        assert_eq!(item.display_competitor_name(), "Alice Smith (2022SMIT01)");
        assert_eq!(item.truncated_competition_name(20), "Very Long Competi...");
    }
}
