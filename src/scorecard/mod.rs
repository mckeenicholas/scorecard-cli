pub mod events;
pub mod model;
pub mod planner;

pub use model::{ScorecardItem, TimeLimitInfo};
pub use planner::{PlannerError, ScorecardPlanner};

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

        assert_eq!(item.display_competitor_name(), "Alice Smith");
        assert_eq!(item.display_wca_id(), "2022SMIT01");
        assert_eq!(item.truncated_competition_name(20), "Very Long Competi...");
        assert_eq!(
            item.formatted_time_limit_info(),
            Some("Cutoff: < 1:00.00 (2 att)  |  Time limit: 5:00.00".to_string())
        );
    }

    #[test]
    fn test_sort_group_cards_station_and_name() {
        let card1 = ScorecardItem {
            competitor_name: "Zack",
            station_number: Some(2),
            ..Default::default()
        };
        let card2 = ScorecardItem {
            competitor_name: "Alice",
            station_number: Some(1),
            ..Default::default()
        };
        let card3 = ScorecardItem {
            competitor_name: "Charlie",
            station_number: None,
            ..Default::default()
        };
        let card4 = ScorecardItem {
            competitor_name: "Bob",
            station_number: None,
            ..Default::default()
        };

        let mut cards = vec![card1, card2, card3, card4];
        planner::sort_group_cards(&mut cards);

        // Station numbers first: 1 (Alice), then 2 (Zack), then no station sorted alphabetically: Bob, Charlie
        assert_eq!(cards[0].competitor_name, "Alice");
        assert_eq!(cards[0].station_number, Some(1));
        assert_eq!(cards[1].competitor_name, "Zack");
        assert_eq!(cards[1].station_number, Some(2));
        assert_eq!(cards[2].competitor_name, "Bob");
        assert_eq!(cards[2].station_number, None);
        assert_eq!(cards[3].competitor_name, "Charlie");
        assert_eq!(cards[3].station_number, None);
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

        let (targets_all, _) = ScorecardPlanner::resolve_all_targets(&comp);
        assert_eq!(targets_all.len(), 1);
        assert_eq!(targets_all[0].event_id, "333");

        let (targets_explicit, notes_explicit) =
            ScorecardPlanner::resolve_targets(&comp, &["333", "333fm"]);
        assert_eq!(targets_explicit.len(), 1);
        assert_eq!(targets_explicit[0].event_id, "333");
        assert_eq!(notes_explicit.len(), 1);
        assert!(notes_explicit[0].contains("333fm"));
    }

    #[test]
    fn test_resolve_targets_and_planning_with_subsequent_round_assignments() {
        use crate::wcif::model::{
            Activity, Assignment, Competition, Event, Person, Registration, Room, Round, Schedule,
            Venue,
        };

        let comp = Competition {
            format_version: Some("1.0".to_string()),
            id: "CompWithR2".to_string(),
            name: "Comp With Round 2".to_string(),
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
                        event_ids: vec!["333".to_string(), "222".to_string()],
                        is_competing: true,
                    }),
                    avatar: None,
                    roles: None,
                    assignments: vec![
                        Assignment {
                            activity_id: 101, // 333-r1-g1
                            station_number: Some(1),
                            assignment_code: Some("competitor".to_string()),
                        },
                        Assignment {
                            activity_id: 102, // 333-r2-g1 (Alice advanced to R2!)
                            station_number: Some(5),
                            assignment_code: Some("competitor".to_string()),
                        },
                        Assignment {
                            activity_id: 201, // 222-r1-g1
                            station_number: Some(2),
                            assignment_code: Some("competitor".to_string()),
                        },
                    ],
                    personal_bests: vec![],
                },
                Person {
                    registrant_id: Some(2),
                    name: "Bob Jones".to_string(),
                    wca_id: Some("2021JONE01".to_string()),
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
                    assignments: vec![
                        Assignment {
                            activity_id: 101, // 333-r1-g1
                            station_number: Some(2),
                            assignment_code: Some("competitor".to_string()),
                        },
                        // Bob did not advance to 333-r2!
                    ],
                    personal_bests: vec![],
                },
            ],
            events: vec![
                Event {
                    id: "333".to_string(),
                    rounds: vec![
                        Round {
                            id: "333-r1".to_string(),
                            format: Some("a".to_string()),
                            time_limit: None,
                            cutoff: None,
                            advancement_condition: None,
                            scramble_group_count: 1,
                        },
                        Round {
                            id: "333-r2".to_string(),
                            format: Some("a".to_string()),
                            time_limit: None,
                            cutoff: None,
                            advancement_condition: None,
                            scramble_group_count: 1,
                        },
                    ],
                    competitor_limit: None,
                    qualification: None,
                },
                Event {
                    id: "222".to_string(),
                    rounds: vec![
                        Round {
                            id: "222-r1".to_string(),
                            format: Some("a".to_string()),
                            time_limit: None,
                            cutoff: None,
                            advancement_condition: None,
                            scramble_group_count: 1,
                        },
                        Round {
                            id: "222-r2".to_string(),
                            format: Some("a".to_string()),
                            time_limit: None,
                            cutoff: None,
                            advancement_condition: None,
                            scramble_group_count: 1,
                        },
                    ],
                    competitor_limit: None,
                    qualification: None,
                },
            ],
            schedule: Some(Schedule {
                start_date: None,
                number_of_days: None,
                venues: vec![Venue {
                    id: Some(1),
                    name: Some("Main Venue".to_string()),
                    rooms: vec![Room {
                        id: Some(1),
                        name: Some("Main Stage".to_string()),
                        color: None,
                        activities: vec![
                            Activity {
                                id: 101,
                                name: "3x3x3 Round 1 Group 1".to_string(),
                                activity_code: "333-r1-g1".to_string(),
                                start_time: None,
                                end_time: None,
                                child_activities: vec![],
                                scramble_set_id: None,
                            },
                            Activity {
                                id: 102,
                                name: "3x3x3 Round 2 Group 1".to_string(),
                                activity_code: "333-r2-g1".to_string(),
                                start_time: None,
                                end_time: None,
                                child_activities: vec![],
                                scramble_set_id: None,
                            },
                            Activity {
                                id: 201,
                                name: "2x2x2 Round 1 Group 1".to_string(),
                                activity_code: "222-r1-g1".to_string(),
                                start_time: None,
                                end_time: None,
                                child_activities: vec![],
                                scramble_set_id: None,
                            },
                        ],
                    }],
                }],
            }),
            extensions: vec![],
        };

        // 1. When requested_events is empty, it includes:
        // - 333-r1 (Round 1)
        // - 333-r2 (Round 2 with competitor assignments)
        // - 222-r1 (Round 1)
        // (222-r2 is omitted because it has no competitor assignments)
        let (targets_all, _) = ScorecardPlanner::resolve_all_targets(&comp);
        assert_eq!(targets_all.len(), 3);
        assert_eq!(targets_all[0].round_id, "333-r1");
        assert_eq!(targets_all[1].round_id, "333-r2");
        assert_eq!(targets_all[2].round_id, "222-r1");

        // 2. Planning 333-r2 produces a NAMED scorecard for Alice (and NOT Bob):
        let plan_r2 = ScorecardPlanner::plan(&comp, &["333-r2"], false, &[], true, false).unwrap();
        assert_eq!(plan_r2.len(), 1);
        let alice_r2 = &plan_r2[0];
        assert_eq!(alice_r2.competitor_name, "Alice Smith");
        assert_eq!(alice_r2.round_number, 2);
        assert_eq!(alice_r2.station_number, Some(5));

        // 3. Planning 222-r2 explicitly (no assignments) produces blank scorecards:
        let plan_222_r2 =
            ScorecardPlanner::plan(&comp, &["222-r2"], false, &[], true, false).unwrap();
        assert!(!plan_222_r2.is_empty());
        assert!(plan_222_r2[0].is_blank);
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
        let cards_r1 = ScorecardPlanner::plan(&comp, &["333-r1"], false, &[], true, false).unwrap();
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
        let cards_r2 = ScorecardPlanner::plan(&comp, &["333-r2"], false, &[], true, false).unwrap();
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
        use crate::options::CoverSheetBy;
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

        // 1. When all three criteria are enabled (Stage, Group, Round):
        // Additive cover sheets: Round (highest tier) -> Group -> Stage -> cards
        let plan_all = ScorecardPlanner::plan(
            &comp,
            &["333-r1"],
            true,
            &[
                CoverSheetBy::Stage,
                CoverSheetBy::Group,
                CoverSheetBy::Round,
            ],
            true,
            false,
        )
        .unwrap();
        // 3 cover sheets (Round, Group, Stage) + 2 competitor cards = 5 items
        assert_eq!(plan_all.len(), 5);

        // Item 0 is Tier 1 (Round cover sheet)
        let cover_round = &plan_all[0];
        assert!(cover_round.is_cover_sheet);
        assert_eq!(cover_round.scorecard_number, 0);
        assert_eq!(cover_round.group_number, 0);
        assert_eq!(cover_round.stage_name, None);
        assert_eq!(cover_round.total_group_cards, 2);

        // Item 1 is Tier 2 (Group cover sheet)
        let cover_group = &plan_all[1];
        assert!(cover_group.is_cover_sheet);
        assert_eq!(cover_group.scorecard_number, 0);
        assert_eq!(cover_group.group_number, 1);
        assert_eq!(cover_group.stage_name, None);
        assert_eq!(cover_group.total_group_cards, 2);

        // Item 2 is Tier 3 (Stage cover sheet: per group on each stage)
        let cover_stage = &plan_all[2];
        assert!(cover_stage.is_cover_sheet);
        assert_eq!(cover_stage.scorecard_number, 0);
        assert_eq!(cover_stage.group_number, 1);
        assert_eq!(cover_stage.stage_name, Some("Main Hall"));
        assert_eq!(cover_stage.total_group_cards, 2);

        // Item 3 and Item 4 are competitor cards
        let card1 = &plan_all[3];
        assert!(!card1.is_cover_sheet);
        assert_eq!(card1.scorecard_number, 1);
        assert_eq!(card1.competitor_name, "Alice Smith");

        let card2 = &plan_all[4];
        assert!(!card2.is_cover_sheet);
        assert_eq!(card2.scorecard_number, 2);
        assert_eq!(card2.competitor_name, "Bob Jones");

        // 2. When only Stage is enabled (default -c):
        let plan_stage = ScorecardPlanner::plan(
            &comp,
            &["333-r1"],
            true,
            &[CoverSheetBy::Stage],
            true,
            false,
        )
        .unwrap();
        assert_eq!(plan_stage.len(), 3);
        assert!(plan_stage[0].is_cover_sheet);
        assert_eq!(plan_stage[0].group_number, 1);
        assert_eq!(plan_stage[0].stage_name, Some("Main Hall"));

        // 3. When only Group is enabled (-c g):
        let plan_g = ScorecardPlanner::plan(
            &comp,
            &["333-r1"],
            true,
            &[CoverSheetBy::Group],
            true,
            false,
        )
        .unwrap();
        assert_eq!(plan_g.len(), 3);
        assert!(plan_g[0].is_cover_sheet);
        assert_eq!(plan_g[0].group_number, 1);
        assert_eq!(plan_g[0].stage_name, None);

        // 4. When only Round is enabled (-c r):
        let plan_r = ScorecardPlanner::plan(
            &comp,
            &["333-r1"],
            true,
            &[CoverSheetBy::Round],
            true,
            false,
        )
        .unwrap();
        assert_eq!(plan_r.len(), 3);
        assert!(plan_r[0].is_cover_sheet);
        assert_eq!(plan_r[0].group_number, 0);
        assert_eq!(plan_r[0].stage_name, None);
    }

    #[test]
    fn test_planner_additive_multi_stage_multi_group() {
        use crate::options::CoverSheetBy;
        use crate::wcif::model::{
            Activity, Assignment, Competition, Event, Person, Registration, Room, Round, Schedule,
            Venue,
        };

        let comp = Competition {
            format_version: Some("1.0".to_string()),
            id: "MultiStageComp2026".to_string(),
            name: "Multi Stage Comp 2026".to_string(),
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
                        activity_id: 101, // G1 on Blue Stage
                        station_number: Some(1),
                        assignment_code: Some("competitor".to_string()),
                    }],
                    personal_bests: vec![],
                },
                Person {
                    registrant_id: Some(2),
                    name: "Bob Jones".to_string(),
                    wca_id: Some("2021JONE01".to_string()),
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
                        activity_id: 102, // G1 on Red Stage
                        station_number: Some(1),
                        assignment_code: Some("competitor".to_string()),
                    }],
                    personal_bests: vec![],
                },
                Person {
                    registrant_id: Some(3),
                    name: "Charlie Brown".to_string(),
                    wca_id: Some("2020BROW01".to_string()),
                    country_iso2: Some("US".to_string()),
                    gender: Some("m".to_string()),
                    registration: Some(Registration {
                        id: Some(3),
                        status: Some("accepted".to_string()),
                        event_ids: vec!["333".to_string()],
                        is_competing: true,
                    }),
                    avatar: None,
                    roles: None,
                    assignments: vec![Assignment {
                        activity_id: 103, // G2 on Red Stage
                        station_number: Some(1),
                        assignment_code: Some("competitor".to_string()),
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
                    scramble_group_count: 2,
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
                    rooms: vec![
                        Room {
                            id: Some(1),
                            name: Some("Blue Stage".to_string()),
                            color: None,
                            activities: vec![Activity {
                                id: 101,
                                name: "3x3x3 Round 1 Group 1".to_string(),
                                activity_code: "333-r1-g1".to_string(),
                                start_time: None,
                                end_time: None,
                                child_activities: vec![],
                                scramble_set_id: None,
                            }],
                        },
                        Room {
                            id: Some(2),
                            name: Some("Red Stage".to_string()),
                            color: None,
                            activities: vec![
                                Activity {
                                    id: 102,
                                    name: "3x3x3 Round 1 Group 1".to_string(),
                                    activity_code: "333-r1-g1".to_string(),
                                    start_time: None,
                                    end_time: None,
                                    child_activities: vec![],
                                    scramble_set_id: None,
                                },
                                Activity {
                                    id: 103,
                                    name: "3x3x3 Round 1 Group 2".to_string(),
                                    activity_code: "333-r1-g2".to_string(),
                                    start_time: None,
                                    end_time: None,
                                    child_activities: vec![],
                                    scramble_set_id: None,
                                },
                            ],
                        },
                    ],
                }],
            }),
            extensions: vec![],
        };

        // Plan with all three criteria enabled (in reverse tier order to test sorting)
        let plan = ScorecardPlanner::plan(
            &comp,
            &["333-r1"],
            true,
            &[
                CoverSheetBy::Stage,
                CoverSheetBy::Group,
                CoverSheetBy::Round,
            ],
            true,
            false,
        )
        .unwrap();

        // Total 9 items:
        // 0: Round cover sheet (3 total)
        // 1: Group 1 cover sheet (2 total)
        // 2: Group 1 (Blue Stage) cover sheet (1 card)
        // 3: Alice card
        // 4: Group 1 (Red Stage) cover sheet (1 card)
        // 5: Bob card
        // 6: Group 2 cover sheet (1 total)
        // 7: Group 2 (Red Stage) cover sheet (1 card)
        // 8: Charlie card
        assert_eq!(plan.len(), 9);

        // 0: Event cover sheet
        assert!(plan[0].is_cover_sheet);
        assert_eq!(plan[0].group_number, 0);
        assert_eq!(plan[0].stage_name, None);
        assert_eq!(plan[0].total_group_cards, 3);

        // 1: Group 1 cover sheet
        assert!(plan[1].is_cover_sheet);
        assert_eq!(plan[1].group_number, 1);
        assert_eq!(plan[1].stage_name, None);
        assert_eq!(plan[1].total_group_cards, 2);

        // 2: Group 1 Blue Stage cover sheet
        assert!(plan[2].is_cover_sheet);
        assert_eq!(plan[2].group_number, 1);
        assert_eq!(plan[2].stage_name, Some("Blue Stage"));
        assert_eq!(plan[2].total_group_cards, 1);

        // 3: Alice card
        assert!(!plan[3].is_cover_sheet);
        assert_eq!(plan[3].competitor_name, "Alice Smith");
        assert_eq!(plan[3].scorecard_number, 1);

        // 4: Group 1 Red Stage cover sheet
        assert!(plan[4].is_cover_sheet);
        assert_eq!(plan[4].group_number, 1);
        assert_eq!(plan[4].stage_name, Some("Red Stage"));
        assert_eq!(plan[4].total_group_cards, 1);

        // 5: Bob card
        assert!(!plan[5].is_cover_sheet);
        assert_eq!(plan[5].competitor_name, "Bob Jones");
        assert_eq!(plan[5].scorecard_number, 2);

        // 6: Group 2 cover sheet
        assert!(plan[6].is_cover_sheet);
        assert_eq!(plan[6].group_number, 2);
        assert_eq!(plan[6].stage_name, None);
        assert_eq!(plan[6].total_group_cards, 1);

        // 7: Group 2 Red Stage cover sheet
        assert!(plan[7].is_cover_sheet);
        assert_eq!(plan[7].group_number, 2);
        assert_eq!(plan[7].stage_name, Some("Red Stage"));
        assert_eq!(plan[7].total_group_cards, 1);

        // 8: Charlie card
        assert!(!plan[8].is_cover_sheet);
        assert_eq!(plan[8].competitor_name, "Charlie Brown");
        assert_eq!(plan[8].scorecard_number, 3);
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
        assert!(formatted.contains("Event      Round      Status       Competitors"));
        assert!(formatted.contains("333        1          Open         2"));
        assert!(formatted.contains("333        2          Subsequent   16 blank (top 16 ranking)"));
    }

    #[test]
    fn test_format_competitor_name_and_print_one_name() {
        use crate::scorecard::planner::format_competitor_name;

        // When print_one_name is false: preserves original full name
        assert_eq!(
            format_competitor_name("Zhang San (张三)", false),
            "Zhang San (张三)"
        );
        assert_eq!(
            format_competitor_name("Lucas Burliga (Łukasz Burliga)", false),
            "Lucas Burliga (Łukasz Burliga)"
        );
        assert_eq!(format_competitor_name("Alice Smith", false), "Alice Smith");

        // When print_one_name is true: strips parenthesized local or Latin name
        assert_eq!(
            format_competitor_name("Zhang San (张三)", true),
            "Zhang San"
        );
        assert_eq!(
            format_competitor_name("Lucas Burliga (Łukasz Burliga)", true),
            "Lucas Burliga"
        );
        assert_eq!(format_competitor_name("Alice Smith", true), "Alice Smith");
        assert_eq!(
            format_competitor_name("Kim Min-jun (김민준)", true),
            "Kim Min-jun"
        );
    }
}
