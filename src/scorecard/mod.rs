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
        assert_eq!(events::event_name_by_id("333"), Some("3x3x3 Cube"));
        assert_eq!(events::event_name_by_id("222"), Some("2x2x2 Cube"));
        assert_eq!(events::event_name_by_id("333bf"), Some("3x3x3 Blindfolded"));
        assert_eq!(events::event_name_by_id("sq1"), Some("Square-1"));
        assert_eq!(events::event_name_by_id("invalid_event"), None);
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
            stage_name: Some("Red Stage"),
            competitor_name: "Alice Smith",
            registrant_id: Some(10),
            wca_id: Some("2022SMIT01"),
            attempt_count: 5,
            time_limit_info: Some("Cutoff: < 1:00.00 (2 att)  |  Time limit: 5:00.00".to_string()),
            is_blank: false,
        };

        assert_eq!(item.display_competitor_name(), "Alice Smith (2022SMIT01)");
        assert_eq!(item.truncated_competition_name(20), "Very Long Competi...");
    }

    #[test]
    fn test_format_limit_and_cutoff() {
        use crate::wcif::{Cutoff, TimeLimit};

        let tl = TimeLimit {
            centiseconds: 60000,
            cumulative_round_ids: None,
        };
        let cutoff = Cutoff {
            number_of_attempts: 2,
            attempt_result: 4500,
        };

        assert_eq!(
            planner::format_limit_and_cutoff(Some(&tl), Some(&cutoff)),
            Some("Cutoff: < 45.00 (2 att)  |  Time limit: 10:00.00".to_string())
        );

        let tl_cum = TimeLimit {
            centiseconds: 30000,
            cumulative_round_ids: Some(vec!["333bf-r1".to_string()]),
        };
        assert_eq!(
            planner::format_limit_and_cutoff(Some(&tl_cum), None),
            Some("Time limit: 5:00.00 cumulative".to_string())
        );

        assert_eq!(planner::format_limit_and_cutoff(None, None), None);
    }

    #[test]
    fn test_resolve_targets_omits_333fm() {
        use crate::wcif::{Competition, Event, Round};

        let comp = Competition {
            format_version: Some("1.0".to_string()),
            id: "TestComp".to_string(),
            name: "Test Competition 2026".to_string(),
            short_name: None,
            persons: vec![],
            events: vec![
                Event {
                    id: "333".to_string(),
                    rounds: vec![Round {
                        id: "333-r1".to_string(),
                        format: Some("a".to_string()),
                        time_limit: None,
                        cutoff: None,
                        advancement_condition: None,
                        scramble_group_count: 1,
                    }],
                    competitor_limit: None,
                    qualification: None,
                },
                Event {
                    id: "333fm".to_string(),
                    rounds: vec![Round {
                        id: "333fm-r1".to_string(),
                        format: Some("m".to_string()),
                        time_limit: None,
                        cutoff: None,
                        advancement_condition: None,
                        scramble_group_count: 1,
                    }],
                    competitor_limit: None,
                    qualification: None,
                },
            ],
            schedule: None,
            extensions: vec![],
        };

        let targets_all = ScorecardPlanner::resolve_targets(&comp, &[]);
        assert_eq!(targets_all.len(), 1);
        assert_eq!(targets_all[0].event_id, "333");

        let targets_explicit =
            ScorecardPlanner::resolve_targets(&comp, &["333".to_string(), "333fm".to_string()]);
        assert_eq!(targets_explicit.len(), 1);
        assert_eq!(targets_explicit[0].event_id, "333");
    }

    #[test]
    fn test_scorecard_planner_full_pipeline() {
        use crate::wcif::{
            Competition, Cutoff, Event, Round, TimeLimit,
            model::{
                Activity, AdvancementCondition, Assignment, Person, Registration, Room, Schedule,
                Venue,
            },
        };

        let comp = Competition {
            format_version: Some("1.0".to_string()),
            id: "TestComp".to_string(),
            name: "Test Competition 2026".to_string(),
            short_name: Some("Test Comp".to_string()),
            persons: vec![
                Person {
                    registrant_id: Some(1),
                    name: "Alice Smith".to_string(),
                    wca_id: Some("2022SMIT01".to_string()),
                    country_iso2: None,
                    gender: None,
                    registration: Some(Registration {
                        id: Some(1),
                        event_ids: vec!["333".to_string()],
                        status: Some("accepted".to_string()),
                        is_competing: true,
                    }),
                    avatar: None,
                    roles: None,
                    assignments: vec![Assignment {
                        activity_id: 1011,
                        station_number: Some(5),
                        assignment_code: Some("competitor".to_string()),
                    }],
                    personal_bests: vec![],
                },
                Person {
                    registrant_id: Some(2),
                    name: "Bob Jones".to_string(),
                    wca_id: None,
                    country_iso2: None,
                    gender: None,
                    registration: Some(Registration {
                        id: Some(2),
                        event_ids: vec!["333".to_string()],
                        status: Some("pending".to_string()), // Not accepted!
                        is_competing: true,
                    }),
                    avatar: None,
                    roles: None,
                    assignments: vec![],
                    personal_bests: vec![],
                },
            ],
            events: vec![Event {
                id: "333".to_string(),
                rounds: vec![
                    Round {
                        id: "333-r1".to_string(),
                        format: Some("a".to_string()),
                        time_limit: Some(TimeLimit {
                            centiseconds: 60000,
                            cumulative_round_ids: None,
                        }),
                        cutoff: Some(Cutoff {
                            number_of_attempts: 2,
                            attempt_result: 4500,
                        }),
                        advancement_condition: Some(AdvancementCondition {
                            condition_type: "ranking".to_string(),
                            value: Some(1.0),
                        }),
                        scramble_group_count: 1,
                    },
                    Round {
                        id: "333-r2".to_string(),
                        format: Some("a".to_string()),
                        time_limit: Some(TimeLimit {
                            centiseconds: 60000,
                            cumulative_round_ids: None,
                        }),
                        cutoff: None,
                        advancement_condition: None,
                        scramble_group_count: 1,
                    },
                ],
                competitor_limit: None,
                qualification: None,
            }],
            schedule: Some(Schedule {
                start_date: Some("2026-06-01".to_string()),
                number_of_days: Some(1),
                venues: vec![Venue {
                    id: Some(1),
                    name: Some("Main Venue".to_string()),
                    rooms: vec![Room {
                        id: Some(10),
                        name: Some("Red Stage".to_string()),
                        color: None,
                        activities: vec![Activity {
                            id: 101,
                            name: "3x3x3 Round 1".to_string(),
                            activity_code: "333-r1".to_string(),
                            start_time: None,
                            end_time: None,
                            child_activities: vec![Activity {
                                id: 1011,
                                name: "3x3x3 Round 1 Group 1".to_string(),
                                activity_code: "333-r1-g1".to_string(),
                                start_time: None,
                                end_time: None,
                                child_activities: vec![],
                                scramble_set_id: None,
                            }],
                            scramble_set_id: None,
                        }],
                    }],
                }],
            }),
            extensions: vec![],
        };

        // Plan Round 1
        let cards_r1 = ScorecardPlanner::plan(&comp, &["333-r1".to_string()]).unwrap();
        assert_eq!(cards_r1.len(), 1);
        let card1 = &cards_r1[0];
        assert_eq!(card1.scorecard_number, 1);
        assert_eq!(card1.competitor_name, "Alice Smith");
        assert_eq!(card1.station_number, Some(5));
        assert_eq!(card1.stage_name, Some("Red Stage"));
        assert_eq!(card1.group_number, 1);
        assert!(!card1.is_blank);
        assert!(
            card1
                .time_limit_info
                .as_ref()
                .unwrap()
                .contains("Cutoff: < 45.00")
        );

        // Plan Round 2 (advancement blanks)
        let cards_r2 = ScorecardPlanner::plan(&comp, &["333-r2".to_string()]).unwrap();
        assert_eq!(cards_r2.len(), 1);
        let blank_card = &cards_r2[0];
        assert_eq!(blank_card.scorecard_number, 1);
        assert!(blank_card.is_blank);
        assert_eq!(blank_card.competitor_name, "");
        assert_eq!(blank_card.station_number, None);
    }
}
