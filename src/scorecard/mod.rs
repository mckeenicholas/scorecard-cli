pub mod events;
pub mod model;
pub mod planner;

pub use model::{ScorecardItem, TimeLimitInfo};
pub use planner::ScorecardPlanner;

#[cfg(test)]
mod tests {
    use super::model::{PlannedRoundSummary, ScorecardPlan};
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
            time_limit_info: Some(TimeLimitInfo {
                limit_centiseconds: Some(30000),
                is_cumulative: false,
                cutoff_centiseconds: Some(6000),
                cutoff_attempts: 2,
            }),
            is_blank: false,
            is_cover_sheet: false,
            total_group_cards: 0,
        };

        assert_eq!(item.display_competitor_name(), "Alice Smith (2022SMIT01)");
        assert_eq!(item.truncated_competition_name(20), "Very Long Competi...");
        assert_eq!(
            item.formatted_time_limit_info(),
            Some("Cutoff: < 1:00.00 (2 att)  |  Time limit: 5:00.00".to_string())
        );
    }

    #[test]
    fn test_time_limit_info_from_wcif_and_format() {
        use crate::wcif::{Cutoff, TimeLimit};

        let tl = TimeLimit {
            centiseconds: 60000,
            cumulative_round_ids: None,
        };
        let cutoff = Cutoff {
            number_of_attempts: 2,
            attempt_result: 4500,
        };

        let info = TimeLimitInfo::from_wcif(Some(&tl), Some(&cutoff)).unwrap();
        assert_eq!(info.limit_centiseconds, Some(60000));
        assert_eq!(info.cutoff_centiseconds, Some(4500));
        assert_eq!(info.cutoff_attempts, 2);
        assert!(!info.is_cumulative);
        assert_eq!(
            info.format_display(),
            "Cutoff: < 45.00 (2 att)  |  Time limit: 10:00.00"
        );

        let tl_cum = TimeLimit {
            centiseconds: 30000,
            cumulative_round_ids: Some(vec!["333bf-r1".to_string()]),
        };
        let info_cum = TimeLimitInfo::from_wcif(Some(&tl_cum), None).unwrap();
        assert!(info_cum.is_cumulative);
        assert_eq!(info_cum.format_display(), "Time limit: 5:00.00 cumulative");

        assert_eq!(TimeLimitInfo::from_wcif(None, None), None);
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

        let (targets_all, _) = ScorecardPlanner::resolve_targets(&comp, &[]);
        assert_eq!(targets_all.len(), 1);
        assert_eq!(targets_all[0].event_id, "333");

        let (targets_explicit, notes_explicit) =
            ScorecardPlanner::resolve_targets(&comp, &["333".to_string(), "333fm".to_string()]);
        assert_eq!(targets_explicit.len(), 1);
        assert_eq!(targets_explicit[0].event_id, "333");
        assert_eq!(notes_explicit.len(), 1);
        assert!(notes_explicit[0].contains("333fm"));
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
        let cards_r1 = ScorecardPlanner::plan(&comp, &["333-r1".to_string()], false).unwrap();
        assert_eq!(cards_r1.len(), 1);
        let card1 = &cards_r1[0];
        assert_eq!(card1.scorecard_number, 1);
        assert_eq!(card1.competitor_name, "Alice Smith");
        assert_eq!(card1.station_number, Some(5));
        assert_eq!(card1.stage_name, Some("Red Stage"));
        assert_eq!(card1.group_number, 1);
        assert!(!card1.is_blank);
        assert!(!card1.is_cover_sheet);
        assert_eq!(
            card1.formatted_time_limit_info().as_deref(),
            Some("Cutoff: < 45.00 (2 att)  |  Time limit: 10:00.00")
        );
        assert_eq!(
            card1.time_limit_info.unwrap().cutoff_centiseconds,
            Some(4500)
        );
        assert_eq!(
            card1.time_limit_info.unwrap().limit_centiseconds,
            Some(60000)
        );

        // Plan Round 2 (advancement blanks)
        let cards_r2 = ScorecardPlanner::plan(&comp, &["333-r2".to_string()], false).unwrap();
        assert_eq!(cards_r2.len(), 1);
        let blank_card = &cards_r2[0];
        assert_eq!(blank_card.scorecard_number, 1);
        assert!(blank_card.is_blank);
        assert!(!blank_card.is_cover_sheet);
        assert_eq!(blank_card.competitor_name, "");
        assert_eq!(blank_card.station_number, None);
    }

    #[test]
    fn test_planner_with_cover_sheets() {
        use crate::wcif::model::{
            Activity, Assignment, Competition, Event, Person, Registration, Room, Round, Schedule,
            Venue,
        };

        let comp = Competition {
            format_version: Some("1.0".to_string()),
            id: "OceanState2025".to_string(),
            name: "Ocean State Cubikon 2025".to_string(),
            short_name: None,
            persons: vec![
                Person {
                    registrant_id: Some(1),
                    name: "Alice Smith".to_string(),
                    wca_id: Some("2022SMIT01".to_string()),
                    country_iso2: Some("US".to_string()),
                    gender: Some("f".to_string()),
                    registration: Some(Registration {
                        id: Some(1),
                        status: Some("accepted".to_string()),
                        event_ids: vec!["333".to_string()],
                        is_competing: true,
                    }),
                    avatar: None,
                    roles: None,
                    assignments: vec![Assignment {
                        activity_id: 1011,
                        assignment_code: Some("competitor".to_string()),
                        station_number: Some(1),
                    }],
                    personal_bests: vec![],
                },
                Person {
                    registrant_id: Some(2),
                    name: "Bob Jones".to_string(),
                    wca_id: None,
                    country_iso2: Some("US".to_string()),
                    gender: Some("m".to_string()),
                    registration: Some(Registration {
                        id: Some(2),
                        status: Some("accepted".to_string()),
                        event_ids: vec!["333".to_string()],
                        is_competing: true,
                    }),
                    avatar: None,
                    roles: None,
                    assignments: vec![Assignment {
                        activity_id: 1011,
                        assignment_code: Some("competitor".to_string()),
                        station_number: Some(2),
                    }],
                    personal_bests: vec![],
                },
            ],
            events: vec![Event {
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
            }],
            schedule: Some(Schedule {
                start_date: None,
                number_of_days: None,
                venues: vec![Venue {
                    id: Some(1),
                    name: Some("Venue".to_string()),
                    rooms: vec![Room {
                        id: Some(1),
                        name: Some("Main Hall".to_string()),
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

        // When cover sheets are enabled:
        let plan = ScorecardPlanner::plan(&comp, &["333-r1".to_string()], true).unwrap();
        // 1 cover sheet + 2 competitor cards = 3 items
        assert_eq!(plan.len(), 3);

        // Item 0 is the cover sheet
        let cover = &plan[0];
        assert!(cover.is_cover_sheet);
        assert_eq!(cover.scorecard_number, 0);
        assert_eq!(cover.total_group_cards, 2);
        assert_eq!(cover.group_number, 1);
        assert_eq!(cover.stage_name, Some("Main Hall"));
        assert_eq!(cover.event_name, "3x3x3 Cube");

        // Item 1 and Item 2 are competitor cards
        let card1 = &plan[1];
        assert!(!card1.is_cover_sheet);
        assert_eq!(card1.scorecard_number, 1);
        assert_eq!(card1.competitor_name, "Alice Smith");

        let card2 = &plan[2];
        assert!(!card2.is_cover_sheet);
        assert_eq!(card2.scorecard_number, 2);
        assert_eq!(card2.competitor_name, "Bob Jones");
    }

    #[test]
    fn test_scorecard_plan_summary_formatting() {
        let plan = ScorecardPlan {
            items: vec![],
            summaries: vec![
                PlannedRoundSummary::OpenRound {
                    event_id: "333".to_string(),
                    round_number: 1,
                    competitor_count: 2,
                    sample_competitor_names: vec!["Alice".to_string(), "Bob".to_string()],
                },
                PlannedRoundSummary::SubsequentRound {
                    event_id: "333".to_string(),
                    round_number: 2,
                    blank_count: 16,
                    reason: "top 16 ranking".to_string(),
                },
            ],
            notes: vec!["Skipped '333fm'".to_string()],
        };

        let formatted = plan.format_summary();
        assert!(formatted.contains("Note: Skipped '333fm'"));
        assert!(formatted.contains("[333 Round 1] (Open Round)"));
        assert!(formatted.contains("Competitors: Alice, Bob"));
        assert!(
            formatted
                .contains("[333 Round 2] (Subsequent Round) -> Generating 16 blank scorecards")
        );
    }
}
