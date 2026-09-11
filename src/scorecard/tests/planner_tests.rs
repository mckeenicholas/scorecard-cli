use std::num::NonZeroUsize;

use crate::options::CoverSheetBy;
use crate::scorecard::events::{RoundId, WcaEvent};
use crate::scorecard::model::{Competitor, Scorecard, ScorecardItem, WcaResult};
use crate::scorecard::planner::{self, PlanConfig, ScorecardPlanner};
use crate::wcif::model::{
    Activity, AdvancementCondition, Assignment, CountryIso2, Person, PersonalBest, Registration,
    Room, Schedule, Venue,
};
use crate::wcif::{Competition, Cutoff, Event, Round, TimeLimit, WcaId};

#[test]
fn test_sort_group_cards_station_and_name() {
    let card1 = Scorecard::new(
        "Test Comp",
        WcaEvent::E333,
        1,
        1,
        Competitor::simple("Zack"),
    )
    .with_station(Some(2))
    .into();
    let card2 = Scorecard::new(
        "Test Comp",
        WcaEvent::E333,
        1,
        1,
        Competitor::simple("Alice"),
    )
    .with_station(Some(1))
    .into();
    let card3 = Scorecard::new(
        "Test Comp",
        WcaEvent::E333,
        1,
        1,
        Competitor::simple("Charlie"),
    )
    .into();
    let card4 = Scorecard::new("Test Comp", WcaEvent::E333, 1, 1, Competitor::simple("Bob")).into();

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
        format_version: Some("1.0".to_owned()),
        id: "TestComp".to_owned(),
        name: "Test Competition 2026".to_owned(),
        short_name: None,
        persons: vec![],
        events: vec![
            Event {
                id: "333".to_owned(),
                rounds: vec![Round {
                    id: "333-r1".to_owned(),
                    format: Some("a".to_owned()),
                    time_limit: None,
                    cutoff: None,
                    advancement_condition: None,
                    scramble_group_count: 1,
                }],
                competitor_limit: None,
                qualification: None,
            },
            Event {
                id: "333fm".to_owned(),
                rounds: vec![Round {
                    id: "333fm-r1".to_owned(),
                    format: Some("m".to_owned()),
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
    assert_eq!(targets_all[0].round_id.event, WcaEvent::E333);

    let (targets_explicit, notes_explicit) =
        ScorecardPlanner::resolve_targets(&comp, &["333", "333fm"]);
    assert_eq!(targets_explicit.len(), 1);
    assert_eq!(targets_explicit[0].round_id.event, WcaEvent::E333);
    assert_eq!(notes_explicit.len(), 1);
    assert!(notes_explicit[0].contains("333fm"));
}

#[test]
fn test_resolve_targets_and_planning_with_subsequent_round_assignments() {
    let comp = Competition {
        format_version: Some("1.0".to_owned()),
        id: "CompWithR2".to_owned(),
        name: "Comp With Round 2".to_owned(),
        short_name: None,
        persons: vec![
            Person {
                registrant_id: NonZeroUsize::new(1),
                name: "Alice Smith".to_owned(),
                wca_id: WcaId::parse("2022SMIT01"),
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(1),
                    status: Some("accepted".to_owned()),
                    event_ids: vec!["333".to_owned(), "222".to_owned()],
                    is_competing: true,
                }),
                assignments: vec![
                    Assignment {
                        activity_id: 101, // 333-r1-g1
                        station_number: Some(1),
                        code: Some("competitor".to_owned()),
                    },
                    Assignment {
                        activity_id: 102, // 333-r2-g1 (Alice advanced to R2!)
                        station_number: Some(5),
                        code: Some("competitor".to_owned()),
                    },
                    Assignment {
                        activity_id: 201, // 222-r1-g1
                        station_number: Some(2),
                        code: Some("competitor".to_owned()),
                    },
                ],
                personal_bests: vec![],
            },
            Person {
                registrant_id: NonZeroUsize::new(2),
                name: "Bob Jones".to_owned(),
                wca_id: WcaId::parse("2021JONE01"),
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(2),
                    status: Some("accepted".to_owned()),
                    event_ids: vec!["333".to_owned()],
                    is_competing: true,
                }),
                assignments: vec![
                    Assignment {
                        activity_id: 101, // 333-r1-g1
                        station_number: Some(2),
                        code: Some("competitor".to_owned()),
                    },
                    // Bob did not advance to 333-r2!
                ],
                personal_bests: vec![],
            },
        ],
        events: vec![
            Event {
                id: "333".to_owned(),
                rounds: vec![
                    Round {
                        id: "333-r1".to_owned(),
                        format: Some("a".to_owned()),
                        time_limit: None,
                        cutoff: None,
                        advancement_condition: None,
                        scramble_group_count: 1,
                    },
                    Round {
                        id: "333-r2".to_owned(),
                        format: Some("a".to_owned()),
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
                id: "222".to_owned(),
                rounds: vec![
                    Round {
                        id: "222-r1".to_owned(),
                        format: Some("a".to_owned()),
                        time_limit: None,
                        cutoff: None,
                        advancement_condition: None,
                        scramble_group_count: 1,
                    },
                    Round {
                        id: "222-r2".to_owned(),
                        format: Some("a".to_owned()),
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
                name: Some("Main Venue".to_owned()),
                rooms: vec![Room {
                    id: Some(1),
                    name: Some("Main Stage".to_owned()),
                    color: None,
                    activities: vec![
                        Activity {
                            id: 101,
                            name: "3x3x3 Round 1 Group 1".to_owned(),
                            code: "333-r1-g1".to_owned(),
                            start_time: None,
                            end_time: None,
                            child_activities: vec![],
                            scramble_set_id: None,
                        },
                        Activity {
                            id: 102,
                            name: "3x3x3 Round 2 Group 1".to_owned(),
                            code: "333-r2-g1".to_owned(),
                            start_time: None,
                            end_time: None,
                            child_activities: vec![],
                            scramble_set_id: None,
                        },
                        Activity {
                            id: 201,
                            name: "2x2x2 Round 1 Group 1".to_owned(),
                            code: "222-r1-g1".to_owned(),
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
    assert_eq!(targets_all[0].round_id, RoundId::new(WcaEvent::E333, 1));
    assert_eq!(targets_all[1].round_id, RoundId::new(WcaEvent::E333, 2));
    assert_eq!(targets_all[2].round_id, RoundId::new(WcaEvent::E222, 1));

    // 2. Planning 333-r2 produces a NAMED scorecard for Alice (and NOT Bob):
    let plan_r2 = ScorecardPlanner::plan(
        &comp,
        &["333-r2"],
        PlanConfig::new(false, &[], true, false, false),
    )
    .unwrap();
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
    let plan_222_r2 = ScorecardPlanner::plan(
        &comp,
        &["222-r2"],
        PlanConfig::new(false, &[], true, false, false),
    )
    .unwrap();
    assert!(!plan_222_r2.is_empty());
    assert!(plan_222_r2[0].is_blank());
}

#[test]
fn test_scorecard_planner_full_pipeline() {
    let comp = Competition {
        format_version: Some("1.0".to_owned()),
        id: "TestComp".to_owned(),
        name: "Test Competition 2026".to_owned(),
        short_name: Some("Test Comp".to_owned()),
        persons: vec![
            Person {
                registrant_id: NonZeroUsize::new(1),
                name: "Alice Smith".to_owned(),
                wca_id: WcaId::parse("2022SMIT01"),
                country_iso2: None,
                registration: Some(Registration {
                    id: NonZeroUsize::new(1),
                    event_ids: vec!["333".to_owned()],
                    status: Some("accepted".to_owned()),
                    is_competing: true,
                }),
                assignments: vec![Assignment {
                    activity_id: 1011,
                    station_number: Some(5),
                    code: Some("competitor".to_owned()),
                }],
                personal_bests: vec![],
            },
            Person {
                registrant_id: NonZeroUsize::new(2),
                name: "Bob Jones".to_owned(),
                wca_id: None,
                country_iso2: None,
                registration: Some(Registration {
                    id: NonZeroUsize::new(2),
                    event_ids: vec!["333".to_owned()],
                    status: Some("pending".to_owned()), // Not accepted!
                    is_competing: true,
                }),
                assignments: vec![],
                personal_bests: vec![],
            },
        ],
        events: vec![Event {
            id: "333".to_owned(),
            rounds: vec![
                Round {
                    id: "333-r1".to_owned(),
                    format: Some("a".to_owned()),
                    time_limit: Some(TimeLimit {
                        centiseconds: 60000,
                        cumulative_round_ids: None,
                    }),
                    cutoff: Some(Cutoff {
                        number_of_attempts: 2,
                        attempt_result: 4500,
                    }),
                    advancement_condition: Some(AdvancementCondition {
                        condition_type: "ranking".to_owned(),
                        value: Some(1),
                    }),
                    scramble_group_count: 1,
                },
                Round {
                    id: "333-r2".to_owned(),
                    format: Some("a".to_owned()),
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
            start_date: Some("2026-06-01".to_owned()),
            number_of_days: Some(1),
            venues: vec![Venue {
                id: Some(1),
                name: Some("Main Venue".to_owned()),
                rooms: vec![Room {
                    id: Some(10),
                    name: Some("Red Stage".to_owned()),
                    color: None,
                    activities: vec![Activity {
                        id: 101,
                        name: "3x3x3 Round 1".to_owned(),
                        code: "333-r1".to_owned(),
                        start_time: None,
                        end_time: None,
                        child_activities: vec![Activity {
                            id: 1011,
                            name: "3x3x3 Round 1 Group 1".to_owned(),
                            code: "333-r1-g1".to_owned(),
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
    let cards_r1 = ScorecardPlanner::plan(
        &comp,
        &["333-r1"],
        PlanConfig::new(false, &[], true, false, false),
    )
    .unwrap();
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
    let cards_r2 = ScorecardPlanner::plan(
        &comp,
        &["333-r2"],
        PlanConfig::new(false, &[], true, false, false),
    )
    .unwrap();
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
        format_version: Some("1.0".to_owned()),
        id: "OceanState2025".to_owned(),
        name: "Ocean State Cubikon 2025".to_owned(),
        short_name: None,
        persons: vec![
            Person {
                registrant_id: NonZeroUsize::new(1),
                name: "Alice Smith".to_owned(),
                wca_id: WcaId::parse("2022SMIT01"),
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(1),
                    status: Some("accepted".to_owned()),
                    event_ids: vec!["333".to_owned()],
                    is_competing: true,
                }),
                assignments: vec![Assignment {
                    activity_id: 1011,
                    code: Some("competitor".to_owned()),
                    station_number: Some(1),
                }],
                personal_bests: vec![],
            },
            Person {
                registrant_id: NonZeroUsize::new(2),
                name: "Bob Jones".to_owned(),
                wca_id: None,
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(2),
                    status: Some("accepted".to_owned()),
                    event_ids: vec!["333".to_owned()],
                    is_competing: true,
                }),
                assignments: vec![Assignment {
                    activity_id: 1011,
                    code: Some("competitor".to_owned()),
                    station_number: Some(2),
                }],
                personal_bests: vec![],
            },
        ],
        events: vec![Event {
            id: "333".to_owned(),
            rounds: vec![Round {
                id: "333-r1".to_owned(),
                format: Some("a".to_owned()),
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
                name: Some("Venue".to_owned()),
                rooms: vec![Room {
                    id: Some(1),
                    name: Some("Main Hall".to_owned()),
                    color: None,
                    activities: vec![Activity {
                        id: 101,
                        name: "3x3x3 Round 1".to_owned(),
                        code: "333-r1".to_owned(),
                        start_time: None,
                        end_time: None,
                        child_activities: vec![Activity {
                            id: 1011,
                            name: "3x3x3 Round 1 Group 1".to_owned(),
                            code: "333-r1-g1".to_owned(),
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
        PlanConfig::new(
            true,
            &[
                CoverSheetBy::Stage,
                CoverSheetBy::Group,
                CoverSheetBy::Round,
            ],
            true,
            false,
            false,
        ),
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
        PlanConfig::new(true, &[CoverSheetBy::Stage], true, false, false),
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
        PlanConfig::new(true, &[CoverSheetBy::Group], true, false, false),
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
        PlanConfig::new(true, &[CoverSheetBy::Round], true, false, false),
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
        format_version: Some("1.0".to_owned()),
        id: "MultiStageComp2026".to_owned(),
        name: "Multi Stage Comp 2026".to_owned(),
        short_name: None,
        persons: vec![
            Person {
                registrant_id: NonZeroUsize::new(1),
                name: "Alice Smith".to_owned(),
                wca_id: WcaId::parse("2022SMIT01"),
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(1),
                    status: Some("accepted".to_owned()),
                    event_ids: vec!["333".to_owned()],
                    is_competing: true,
                }),
                assignments: vec![Assignment {
                    activity_id: 101, // G1 on Blue Stage
                    station_number: Some(1),
                    code: Some("competitor".to_owned()),
                }],
                personal_bests: vec![],
            },
            Person {
                registrant_id: NonZeroUsize::new(2),
                name: "Bob Jones".to_owned(),
                wca_id: WcaId::parse("2021JONE01"),
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(2),
                    status: Some("accepted".to_owned()),
                    event_ids: vec!["333".to_owned()],
                    is_competing: true,
                }),
                assignments: vec![Assignment {
                    activity_id: 102, // G1 on Red Stage
                    station_number: Some(1),
                    code: Some("competitor".to_owned()),
                }],
                personal_bests: vec![],
            },
            Person {
                registrant_id: NonZeroUsize::new(3),
                name: "Charlie Brown".to_owned(),
                wca_id: WcaId::parse("2020BROW01"),
                country_iso2: CountryIso2::parse("US"),
                registration: Some(Registration {
                    id: NonZeroUsize::new(3),
                    status: Some("accepted".to_owned()),
                    event_ids: vec!["333".to_owned()],
                    is_competing: true,
                }),
                assignments: vec![Assignment {
                    activity_id: 103, // G2 on Red Stage
                    station_number: Some(1),
                    code: Some("competitor".to_owned()),
                }],
                personal_bests: vec![],
            },
        ],
        events: vec![Event {
            id: "333".to_owned(),
            rounds: vec![Round {
                id: "333-r1".to_owned(),
                format: Some("a".to_owned()),
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
                name: Some("Venue".to_owned()),
                rooms: vec![
                    Room {
                        id: Some(1),
                        name: Some("Blue Stage".to_owned()),
                        color: None,
                        activities: vec![Activity {
                            id: 101,
                            name: "3x3x3 Round 1 Group 1".to_owned(),
                            code: "333-r1-g1".to_owned(),
                            start_time: None,
                            end_time: None,
                            child_activities: vec![],
                            scramble_set_id: None,
                        }],
                    },
                    Room {
                        id: Some(2),
                        name: Some("Red Stage".to_owned()),
                        color: None,
                        activities: vec![
                            Activity {
                                id: 102,
                                name: "3x3x3 Round 1 Group 1".to_owned(),
                                code: "333-r1-g1".to_owned(),
                                start_time: None,
                                end_time: None,
                                child_activities: vec![],
                                scramble_set_id: None,
                            },
                            Activity {
                                id: 103,
                                name: "3x3x3 Round 1 Group 2".to_owned(),
                                code: "333-r1-g2".to_owned(),
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
        PlanConfig::new(
            true,
            &[
                CoverSheetBy::Stage,
                CoverSheetBy::Group,
                CoverSheetBy::Round,
            ],
            true,
            false,
            false,
        ),
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

#[test]
fn test_planner_local_names_first() {
    let comp = Competition {
        format_version: Some("1.0".to_owned()),
        id: "LocalNameComp2026".to_owned(),
        name: "Local Name Comp 2026".to_owned(),
        short_name: Some("Local Name 2026".to_owned()),
        persons: vec![Person {
            registrant_id: NonZeroUsize::new(1),
            name: "Zhang San (张三)".to_owned(),
            wca_id: None,
            country_iso2: CountryIso2::parse("CN"),
            registration: Some(Registration {
                id: NonZeroUsize::new(1),
                status: Some("accepted".to_owned()),
                event_ids: vec!["333".to_owned()],
                is_competing: true,
            }),
            assignments: vec![],
            personal_bests: vec![],
        }],
        events: vec![Event {
            id: "333".to_owned(),
            rounds: vec![Round {
                id: "333-r1".to_owned(),
                format: Some("a".to_owned()),
                time_limit: None,
                cutoff: None,
                advancement_condition: None,
                scramble_group_count: 1,
            }],
            competitor_limit: None,
            qualification: None,
        }],
        schedule: None,
        extensions: vec![],
    };

    // With local_names_first = false: primary is "Zhang San", local is Some("张三")
    let plan_standard = ScorecardPlanner::plan(
        &comp,
        &["333-r1"],
        PlanConfig::new(false, &[], false, false, false),
    )
    .unwrap();
    assert_eq!(plan_standard.len(), 1);
    if let ScorecardItem::Scorecard(sc) = &plan_standard[0] {
        assert_eq!(sc.competitor.name, "Zhang San");
        assert_eq!(sc.competitor.local_name, Some("张三"));
    } else {
        panic!("Expected Scorecard");
    }

    // With local_names_first = true: primary is "张三", local is Some("Zhang San")
    let plan_local_first = ScorecardPlanner::plan(
        &comp,
        &["333-r1"],
        PlanConfig::new(false, &[], false, false, true),
    )
    .unwrap();
    assert_eq!(plan_local_first.len(), 1);
    if let ScorecardItem::Scorecard(sc) = &plan_local_first[0] {
        assert_eq!(sc.competitor.name, "张三");
        assert_eq!(sc.competitor.local_name, Some("Zhang San"));
    } else {
        panic!("Expected Scorecard");
    }
}

#[test]
fn test_scramble_checker_top_ranked_and_excluded_events() {
    let make_person = |id: usize, name: &str, pbs: Vec<PersonalBest>| Person {
        registrant_id: NonZeroUsize::new(id),
        name: name.to_owned(),
        wca_id: Some(WcaId::parse(&format!("2020TEST{:02}", id)).unwrap()),
        country_iso2: CountryIso2::parse("US"),
        registration: Some(Registration {
            id: NonZeroUsize::new(id),
            status: Some("accepted".to_owned()),
            event_ids: vec!["333".to_owned(), "555".to_owned()],
            is_competing: true,
        }),
        assignments: vec![],
        personal_bests: pbs,
    };

    // p1: 333 single world rank 50 (qualifies <= 50)
    let p1 = make_person(
        1,
        "Single WR 50",
        vec![PersonalBest {
            event_id: "333".to_owned(),
            best_type: "single".to_owned(),
            best: Some(500),
            world_ranking: Some(50),
            national_ranking: Some(10),
            continental_ranking: Some(20),
        }],
    );

    // p2: 333 single world rank 51, no average (does not qualify)
    let p2 = make_person(
        2,
        "Single WR 51",
        vec![PersonalBest {
            event_id: "333".to_owned(),
            best_type: "single".to_owned(),
            best: Some(501),
            world_ranking: Some(51),
            national_ranking: Some(10),
            continental_ranking: Some(20),
        }],
    );

    // p3: 333 average world rank 42 (qualifies <= 50)
    let p3 = make_person(
        3,
        "Average WR 42",
        vec![PersonalBest {
            event_id: "333".to_owned(),
            best_type: "average".to_owned(),
            best: Some(600),
            world_ranking: Some(42),
            national_ranking: Some(20),
            continental_ranking: Some(30),
        }],
    );

    // p4: 333 average national rank 15 (qualifies <= 15 even with world rank 100)
    let p4 = make_person(
        4,
        "Average NR 15",
        vec![PersonalBest {
            event_id: "333".to_owned(),
            best_type: "average".to_owned(),
            best: Some(700),
            world_ranking: Some(100),
            national_ranking: Some(15),
            continental_ranking: Some(50),
        }],
    );

    // p5: 333 average national rank 16 (does not qualify)
    let p5 = make_person(
        5,
        "Average NR 16",
        vec![PersonalBest {
            event_id: "333".to_owned(),
            best_type: "average".to_owned(),
            best: Some(705),
            world_ranking: Some(100),
            national_ranking: Some(16),
            continental_ranking: Some(50),
        }],
    );

    // p6: 555 world rank 1 (single & average) - but 555 is an excluded event!
    let p6 = make_person(
        6,
        "555 WR 1",
        vec![
            PersonalBest {
                event_id: "555".to_owned(),
                best_type: "single".to_owned(),
                best: Some(3500),
                world_ranking: Some(1),
                national_ranking: Some(1),
                continental_ranking: Some(1),
            },
            PersonalBest {
                event_id: "555".to_owned(),
                best_type: "average".to_owned(),
                best: Some(3800),
                world_ranking: Some(1),
                national_ranking: Some(1),
                continental_ranking: Some(1),
            },
        ],
    );

    let comp = Competition {
        format_version: Some("1.0".to_owned()),
        id: "CheckerComp2026".to_owned(),
        name: "Checker Comp 2026".to_owned(),
        short_name: None,
        persons: vec![p1, p2, p3, p4, p5, p6],
        events: vec![
            Event {
                id: "333".to_owned(),
                rounds: vec![Round {
                    id: "333-r1".to_owned(),
                    format: Some("a".to_owned()),
                    time_limit: None,
                    cutoff: None,
                    advancement_condition: None,
                    scramble_group_count: 1,
                }],
                competitor_limit: None,
                qualification: None,
            },
            Event {
                id: "555".to_owned(),
                rounds: vec![Round {
                    id: "555-r1".to_owned(),
                    format: Some("a".to_owned()),
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

    let config = PlanConfig::default().with_scramble_checker(true, false, false);
    let plan = ScorecardPlanner::plan(&comp, &["333-r1", "555-r1"], config).unwrap();

    let check_map: std::collections::HashMap<String, bool> = plan
        .iter()
        .filter_map(|item| match item {
            ScorecardItem::Scorecard(sc) => Some((
                format!("{}:{}", sc.event, sc.competitor.name),
                sc.needs_scramble_checker,
            )),
            _ => None,
        })
        .collect();

    assert_eq!(check_map.get("333:Single WR 50"), Some(&true));
    assert_eq!(check_map.get("333:Single WR 51"), Some(&false));
    assert_eq!(check_map.get("333:Average WR 42"), Some(&true));
    assert_eq!(check_map.get("333:Average NR 15"), Some(&true));
    assert_eq!(check_map.get("333:Average NR 16"), Some(&false));
    // 555 excluded event
    assert_eq!(check_map.get("555:555 WR 1"), Some(&false));
}

#[test]
fn test_scramble_checker_final_rounds_and_blank_scorecards() {
    let comp = Competition {
        format_version: Some("1.0".to_owned()),
        id: "FinalRoundComp2026".to_owned(),
        name: "Final Round Comp 2026".to_owned(),
        short_name: None,
        persons: vec![Person {
            registrant_id: NonZeroUsize::new(1),
            name: "Alice Competitor".to_owned(),
            wca_id: None,
            country_iso2: None,
            registration: Some(Registration {
                id: NonZeroUsize::new(1),
                status: Some("accepted".to_owned()),
                event_ids: vec!["333".to_owned(), "555".to_owned()],
                is_competing: true,
            }),
            assignments: vec![],
            personal_bests: vec![], // No rankings
        }],
        events: vec![
            Event {
                id: "333".to_owned(),
                rounds: vec![
                    Round {
                        id: "333-r1".to_owned(),
                        format: Some("a".to_owned()),
                        time_limit: None,
                        cutoff: None,
                        advancement_condition: None,
                        scramble_group_count: 1,
                    },
                    Round {
                        id: "333-r2".to_owned(),
                        format: Some("a".to_owned()),
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
                id: "555".to_owned(),
                rounds: vec![Round {
                    id: "555-r1".to_owned(),
                    format: Some("a".to_owned()),
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

    // 1. With final_rounds = true: 333-r1 is not final (false), 333-r2 is final (true), 555-r1 is final but excluded (false)
    let config_final = PlanConfig::default().with_scramble_checker(false, true, false);
    let plan =
        ScorecardPlanner::plan(&comp, &["333-r1", "333-r2", "555-r1"], config_final).unwrap();

    let check_map: std::collections::HashMap<String, bool> = plan
        .iter()
        .filter_map(|item| match item {
            ScorecardItem::Scorecard(sc) => Some((
                format!("{}-r{}", sc.event, sc.round_number),
                sc.needs_scramble_checker,
            )),
            ScorecardItem::Blank(b) => Some((
                format!("{}-r{}", b.event, b.round_number),
                b.needs_scramble_checker,
            )),
            _ => None,
        })
        .collect();

    assert_eq!(check_map.get("333-r1"), Some(&false));
    assert_eq!(check_map.get("333-r2"), Some(&true));
    assert_eq!(check_map.get("555-r1"), Some(&false));

    // 2. Blank scorecards:
    // When final_rounds is true, final round 333-r2 gets checker, non-final 333-r1 does not, 555 excluded
    assert!(!planner::should_print_scramble_checker_for_blank(
        &comp.events[0],
        &comp.events[0].rounds[0],
        &config_final,
    ));
    assert!(planner::should_print_scramble_checker_for_blank(
        &comp.events[0],
        &comp.events[0].rounds[1],
        &config_final,
    ));
    assert!(!planner::should_print_scramble_checker_for_blank(
        &comp.events[1],
        &comp.events[1].rounds[0],
        &config_final,
    ));

    // When scramble_checker_blank is true, 333-r1 gets checker, 555 excluded
    let config_blank = PlanConfig::default().with_scramble_checker(false, false, true);
    assert!(planner::should_print_scramble_checker_for_blank(
        &comp.events[0],
        &comp.events[0].rounds[0],
        &config_blank,
    ));
    assert!(!planner::should_print_scramble_checker_for_blank(
        &comp.events[1],
        &comp.events[1].rounds[0],
        &config_blank,
    ));
}
