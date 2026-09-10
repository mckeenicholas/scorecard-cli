use crate::options::CoverSheetBy;
use crate::scorecard::events::WcaEvent;
use crate::scorecard::model::{Competitor, ScorecardItem, WcaResult};
use crate::scorecard::planner::{self, ScorecardPlanner};
use crate::wcif::{
    Competition, Cutoff, Event, Round, TimeLimit, WcaId,
    model::{
        Activity, AdvancementCondition, Assignment, CountryIso2, Person, Registration, Room,
        Schedule, Venue,
    },
};
use std::num::NonZeroUsize;

#[test]
fn test_sort_group_cards_station_and_name() {
    let card1 = ScorecardItem::scorecard(
        "Test Comp",
        WcaEvent::E333,
        1,
        1,
        None,
        Competitor::simple("Zack"),
        Some(2),
        5,
        None,
    );
    let card2 = ScorecardItem::scorecard(
        "Test Comp",
        WcaEvent::E333,
        1,
        1,
        None,
        Competitor::simple("Alice"),
        Some(1),
        5,
        None,
    );
    let card3 = ScorecardItem::scorecard(
        "Test Comp",
        WcaEvent::E333,
        1,
        1,
        None,
        Competitor::simple("Charlie"),
        None,
        5,
        None,
    );
    let card4 = ScorecardItem::scorecard(
        "Test Comp",
        WcaEvent::E333,
        1,
        1,
        None,
        Competitor::simple("Bob"),
        None,
        5,
        None,
    );

    let mut cards = vec![card1, card2, card3, card4];
    planner::sort_group_cards(&mut cards);

    let names_and_stations: Vec<_> = cards
        .iter()
        .map(|c| match c {
            ScorecardItem::Scorecard(sc) => (sc.competitor.name, sc.station_number),
            _ => panic!("Expected Scorecard"),
        })
        .collect();
    assert_eq!(
        names_and_stations,
        vec![
            ("Alice", Some(1)),
            ("Zack", Some(2)),
            ("Bob", None),
            ("Charlie", None)
        ]
    );
}

#[test]
fn test_resolve_targets_omits_333fm() {
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
    assert_eq!(targets_all[0].event, WcaEvent::E333);

    let (targets_explicit, notes_explicit) =
        ScorecardPlanner::resolve_targets(&comp, &["333", "333fm"]);
    assert_eq!(targets_explicit.len(), 1);
    assert_eq!(targets_explicit[0].event, WcaEvent::E333);
    assert_eq!(notes_explicit.len(), 1);
    assert!(notes_explicit[0].contains("333fm"));
}

#[test]
fn test_resolve_targets_and_planning_with_subsequent_round_assignments() {
    let comp = Competition {
        format_version: Some("1.0".to_string()),
        id: "CompWithR2".to_string(),
        name: "Comp With Round 2".to_string(),
        short_name: None,
        persons: vec![
            Person {
                registrant_id: NonZeroUsize::new(1),
                name: "Alice Smith".to_string(),
                wca_id: WcaId::parse("2022SMIT01"),
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(1),
                    status: Some("accepted".to_string()),
                    event_ids: vec!["333".to_string(), "222".to_string()],
                    is_competing: true,
                }),
                assignments: vec![
                    Assignment {
                        activity_id: 101, // 333-r1-g1
                        station_number: Some(1),
                        code: Some("competitor".to_string()),
                    },
                    Assignment {
                        activity_id: 102, // 333-r2-g1 (Alice advanced to R2!)
                        station_number: Some(5),
                        code: Some("competitor".to_string()),
                    },
                    Assignment {
                        activity_id: 201, // 222-r1-g1
                        station_number: Some(2),
                        code: Some("competitor".to_string()),
                    },
                ],
            },
            Person {
                registrant_id: NonZeroUsize::new(2),
                name: "Bob Jones".to_string(),
                wca_id: WcaId::parse("2021JONE01"),
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(2),
                    status: Some("accepted".to_string()),
                    event_ids: vec!["333".to_string()],
                    is_competing: true,
                }),
                assignments: vec![
                    Assignment {
                        activity_id: 101, // 333-r1-g1
                        station_number: Some(2),
                        code: Some("competitor".to_string()),
                    },
                    // Bob did not advance to 333-r2!
                ],
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
                            code: "333-r1-g1".to_string(),
                            start_time: None,
                            end_time: None,
                            child_activities: vec![],
                            scramble_set_id: None,
                        },
                        Activity {
                            id: 102,
                            name: "3x3x3 Round 2 Group 1".to_string(),
                            code: "333-r2-g1".to_string(),
                            start_time: None,
                            end_time: None,
                            child_activities: vec![],
                            scramble_set_id: None,
                        },
                        Activity {
                            id: 201,
                            name: "2x2x2 Round 1 Group 1".to_string(),
                            code: "222-r1-g1".to_string(),
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
    if let ScorecardItem::Scorecard(alice) = alice_r2 {
        assert_eq!(alice.competitor.name, "Alice Smith");
        assert_eq!(alice.round_number, 2);
        assert_eq!(alice.station_number, Some(5));
    } else {
        panic!("Expected Scorecard");
    }

    // 3. Planning 222-r2 explicitly (no assignments) produces blank scorecards:
    let plan_222_r2 = ScorecardPlanner::plan(&comp, &["222-r2"], false, &[], true, false).unwrap();
    assert!(!plan_222_r2.is_empty());
    assert!(plan_222_r2[0].is_blank());
}

#[test]
fn test_scorecard_planner_full_pipeline() {
    let comp = Competition {
        format_version: Some("1.0".to_string()),
        id: "TestComp".to_string(),
        name: "Test Competition 2026".to_string(),
        short_name: Some("Test Comp".to_string()),
        persons: vec![
            Person {
                registrant_id: NonZeroUsize::new(1),
                name: "Alice Smith".to_string(),
                wca_id: WcaId::parse("2022SMIT01"),
                country_iso2: None,
                registration: Some(Registration {
                    id: NonZeroUsize::new(1),
                    event_ids: vec!["333".to_string()],
                    status: Some("accepted".to_string()),
                    is_competing: true,
                }),
                assignments: vec![Assignment {
                    activity_id: 1011,
                    station_number: Some(5),
                    code: Some("competitor".to_string()),
                }],
            },
            Person {
                registrant_id: NonZeroUsize::new(2),
                name: "Bob Jones".to_string(),
                wca_id: None,
                country_iso2: None,
                registration: Some(Registration {
                    id: NonZeroUsize::new(2),
                    event_ids: vec!["333".to_string()],
                    status: Some("pending".to_string()), // Not accepted!
                    is_competing: true,
                }),
                assignments: vec![],
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
                        value: Some(1),
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
                        code: "333-r1".to_string(),
                        start_time: None,
                        end_time: None,
                        child_activities: vec![Activity {
                            id: 1011,
                            name: "3x3x3 Round 1 Group 1".to_string(),
                            code: "333-r1-g1".to_string(),
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
    if let ScorecardItem::Scorecard(sc) = card1 {
        assert_eq!(sc.number, 1);
        assert_eq!(sc.competitor.name, "Alice Smith");
        assert_eq!(sc.station_number, Some(5));
        assert_eq!(sc.stage_name, Some("Red Stage"));
        assert_eq!(sc.group_number, 1);
        assert_eq!(
            sc.formatted_time_limit_info().as_deref(),
            Some("Cutoff: < 45.00 (2 att)  |  Time limit: 10:00.00")
        );
        assert_eq!(
            sc.time_limit_info.as_ref().unwrap().cutoff_centiseconds,
            WcaResult::new(4500)
        );
        assert_eq!(
            sc.time_limit_info.as_ref().unwrap().limit_centiseconds,
            WcaResult::new(60000)
        );
    } else {
        panic!("Expected Scorecard");
    }
    assert!(!card1.is_blank());
    assert!(!card1.is_cover_sheet());

    // Plan Round 2 (advancement blanks)
    let cards_r2 = ScorecardPlanner::plan(&comp, &["333-r2"], false, &[], true, false).unwrap();
    assert_eq!(cards_r2.len(), 1);
    let blank_card = &cards_r2[0];
    if let ScorecardItem::Blank(blank) = blank_card {
        assert_eq!(blank.number, 1);
        assert_eq!(blank.station_number, None);
    } else {
        panic!("Expected Blank");
    }
    assert!(blank_card.is_blank());
    assert!(!blank_card.is_cover_sheet());
}

#[test]
fn test_planner_with_cover_sheets() {
    let comp = Competition {
        format_version: Some("1.0".to_string()),
        id: "OceanState2025".to_string(),
        name: "Ocean State Cubikon 2025".to_string(),
        short_name: None,
        persons: vec![
            Person {
                registrant_id: NonZeroUsize::new(1),
                name: "Alice Smith".to_string(),
                wca_id: WcaId::parse("2022SMIT01"),
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(1),
                    status: Some("accepted".to_string()),
                    event_ids: vec!["333".to_string()],
                    is_competing: true,
                }),
                assignments: vec![Assignment {
                    activity_id: 1011,
                    code: Some("competitor".to_string()),
                    station_number: Some(1),
                }],
            },
            Person {
                registrant_id: NonZeroUsize::new(2),
                name: "Bob Jones".to_string(),
                wca_id: None,
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(2),
                    status: Some("accepted".to_string()),
                    event_ids: vec!["333".to_string()],
                    is_competing: true,
                }),
                assignments: vec![Assignment {
                    activity_id: 1011,
                    code: Some("competitor".to_string()),
                    station_number: Some(2),
                }],
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
                        code: "333-r1".to_string(),
                        start_time: None,
                        end_time: None,
                        child_activities: vec![Activity {
                            id: 1011,
                            name: "3x3x3 Round 1 Group 1".to_string(),
                            code: "333-r1-g1".to_string(),
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
    assert!(cover_round.is_cover_sheet());
    if let ScorecardItem::CoverSheet(cs) = cover_round {
        assert_eq!(cs.group_number, 0);
        assert_eq!(cs.stage_name, None);
        assert_eq!(cs.total_group_cards, 2);
    } else {
        panic!("Expected CoverSheet");
    }

    // Item 1 is Tier 2 (Group cover sheet)
    let cover_group = &plan_all[1];
    assert!(cover_group.is_cover_sheet());
    if let ScorecardItem::CoverSheet(cs) = cover_group {
        assert_eq!(cs.group_number, 1);
        assert_eq!(cs.stage_name, None);
        assert_eq!(cs.total_group_cards, 2);
    } else {
        panic!("Expected CoverSheet");
    }

    // Item 2 is Tier 3 (Stage cover sheet: per group on each stage)
    let cover_stage = &plan_all[2];
    assert!(cover_stage.is_cover_sheet());
    if let ScorecardItem::CoverSheet(cs) = cover_stage {
        assert_eq!(cs.group_number, 1);
        assert_eq!(cs.stage_name, Some("Main Hall"));
        assert_eq!(cs.total_group_cards, 2);
    } else {
        panic!("Expected CoverSheet");
    }

    // Item 3 and Item 4 are competitor cards
    let card1 = &plan_all[3];
    assert!(!card1.is_cover_sheet());
    if let ScorecardItem::Scorecard(sc) = card1 {
        assert_eq!(sc.number, 1);
        assert_eq!(sc.competitor.name, "Alice Smith");
    } else {
        panic!("Expected Scorecard");
    }

    let card2 = &plan_all[4];
    assert!(!card2.is_cover_sheet());
    if let ScorecardItem::Scorecard(sc) = card2 {
        assert_eq!(sc.number, 2);
        assert_eq!(sc.competitor.name, "Bob Jones");
    } else {
        panic!("Expected Scorecard");
    }

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
    assert!(plan_stage[0].is_cover_sheet());
    if let ScorecardItem::CoverSheet(cs) = &plan_stage[0] {
        assert_eq!(cs.group_number, 1);
        assert_eq!(cs.stage_name, Some("Main Hall"));
    } else {
        panic!("Expected CoverSheet");
    }

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
    assert!(plan_g[0].is_cover_sheet());
    if let ScorecardItem::CoverSheet(cs) = &plan_g[0] {
        assert_eq!(cs.group_number, 1);
        assert_eq!(cs.stage_name, None);
    } else {
        panic!("Expected CoverSheet");
    }

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
    assert!(plan_r[0].is_cover_sheet());
    if let ScorecardItem::CoverSheet(cs) = &plan_r[0] {
        assert_eq!(cs.group_number, 0);
        assert_eq!(cs.stage_name, None);
    } else {
        panic!("Expected CoverSheet");
    }
}

#[test]
fn test_planner_additive_multi_stage_multi_group() {
    let comp = Competition {
        format_version: Some("1.0".to_string()),
        id: "MultiStageComp2026".to_string(),
        name: "Multi Stage Comp 2026".to_string(),
        short_name: None,
        persons: vec![
            Person {
                registrant_id: NonZeroUsize::new(1),
                name: "Alice Smith".to_string(),
                wca_id: WcaId::parse("2022SMIT01"),
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(1),
                    status: Some("accepted".to_string()),
                    event_ids: vec!["333".to_string()],
                    is_competing: true,
                }),
                assignments: vec![Assignment {
                    activity_id: 101, // G1 on Blue Stage
                    station_number: Some(1),
                    code: Some("competitor".to_string()),
                }],
            },
            Person {
                registrant_id: NonZeroUsize::new(2),
                name: "Bob Jones".to_string(),
                wca_id: WcaId::parse("2021JONE01"),
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(2),
                    status: Some("accepted".to_string()),
                    event_ids: vec!["333".to_string()],
                    is_competing: true,
                }),
                assignments: vec![Assignment {
                    activity_id: 102, // G1 on Red Stage
                    station_number: Some(1),
                    code: Some("competitor".to_string()),
                }],
            },
            Person {
                registrant_id: NonZeroUsize::new(3),
                name: "Charlie Brown".to_string(),
                wca_id: WcaId::parse("2020BROW01"),
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(3),
                    status: Some("accepted".to_string()),
                    event_ids: vec!["333".to_string()],
                    is_competing: true,
                }),
                assignments: vec![Assignment {
                    activity_id: 103, // G2 on Red Stage
                    station_number: Some(1),
                    code: Some("competitor".to_string()),
                }],
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
                            code: "333-r1-g1".to_string(),
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
                                code: "333-r1-g1".to_string(),
                                start_time: None,
                                end_time: None,
                                child_activities: vec![],
                                scramble_set_id: None,
                            },
                            Activity {
                                id: 103,
                                name: "3x3x3 Round 1 Group 2".to_string(),
                                code: "333-r1-g2".to_string(),
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

    assert_eq!(plan.len(), 9);

    // 0: Event cover sheet
    assert!(plan[0].is_cover_sheet());
    if let ScorecardItem::CoverSheet(cs) = &plan[0] {
        assert_eq!(cs.group_number, 0);
        assert_eq!(cs.stage_name, None);
        assert_eq!(cs.total_group_cards, 3);
    } else {
        panic!("Expected CoverSheet");
    }

    // 1: Group 1 cover sheet
    assert!(plan[1].is_cover_sheet());
    if let ScorecardItem::CoverSheet(cs) = &plan[1] {
        assert_eq!(cs.group_number, 1);
        assert_eq!(cs.stage_name, None);
        assert_eq!(cs.total_group_cards, 2);
    } else {
        panic!("Expected CoverSheet");
    }

    // 2: Group 1 Blue Stage cover sheet
    assert!(plan[2].is_cover_sheet());
    if let ScorecardItem::CoverSheet(cs) = &plan[2] {
        assert_eq!(cs.group_number, 1);
        assert_eq!(cs.stage_name, Some("Blue Stage"));
        assert_eq!(cs.total_group_cards, 1);
    } else {
        panic!("Expected CoverSheet");
    }

    // 3: Alice card
    assert!(!plan[3].is_cover_sheet());
    if let ScorecardItem::Scorecard(sc) = &plan[3] {
        assert_eq!(sc.competitor.name, "Alice Smith");
        assert_eq!(sc.number, 1);
    } else {
        panic!("Expected Scorecard");
    }

    // 4: Group 1 Red Stage cover sheet
    assert!(plan[4].is_cover_sheet());
    if let ScorecardItem::CoverSheet(cs) = &plan[4] {
        assert_eq!(cs.group_number, 1);
        assert_eq!(cs.stage_name, Some("Red Stage"));
        assert_eq!(cs.total_group_cards, 1);
    } else {
        panic!("Expected CoverSheet");
    }

    // 5: Bob card
    assert!(!plan[5].is_cover_sheet());
    if let ScorecardItem::Scorecard(sc) = &plan[5] {
        assert_eq!(sc.competitor.name, "Bob Jones");
        assert_eq!(sc.number, 2);
    } else {
        panic!("Expected Scorecard");
    }

    // 6: Group 2 cover sheet
    assert!(plan[6].is_cover_sheet());
    if let ScorecardItem::CoverSheet(cs) = &plan[6] {
        assert_eq!(cs.group_number, 2);
        assert_eq!(cs.stage_name, None);
        assert_eq!(cs.total_group_cards, 1);
    } else {
        panic!("Expected CoverSheet");
    }

    // 7: Group 2 Red Stage cover sheet
    assert!(plan[7].is_cover_sheet());
    if let ScorecardItem::CoverSheet(cs) = &plan[7] {
        assert_eq!(cs.group_number, 2);
        assert_eq!(cs.stage_name, Some("Red Stage"));
        assert_eq!(cs.total_group_cards, 1);
    } else {
        panic!("Expected CoverSheet");
    }

    // 8: Charlie card
    assert!(!plan[8].is_cover_sheet());
    if let ScorecardItem::Scorecard(sc) = &plan[8] {
        assert_eq!(sc.competitor.name, "Charlie Brown");
        assert_eq!(sc.number, 3);
    } else {
        panic!("Expected Scorecard");
    }
}
