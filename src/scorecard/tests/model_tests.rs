use std::num::NonZeroUsize;

use crate::scorecard::events::WcaEvent;
use crate::scorecard::model::{
    Competitor, InvalidWcaResult, PlannedRoundSummary, ScorecardItem, ScorecardPlan, TimeLimitInfo,
    WcaResult,
};
use crate::scorecard::planner;
use crate::wcif::{Cutoff, TimeLimit, WcaId};

#[test]
fn test_scorecard_item_display_methods() {
    let item = ScorecardItem::scorecard(
        "Very Long Competition Name 2026",
        WcaEvent::E333,
        1,
        1,
        Some("Red Stage"),
        Competitor {
            name: "Alice Smith",
            local_name: None,
            registrant_id: NonZeroUsize::new(10).unwrap(),
            wca_id: WcaId::parse("2022SMIT01"),
        },
        Some(7),
        5,
        Some(TimeLimitInfo {
            limit_centiseconds: WcaResult::new(30000),
            is_cumulative: false,
            cutoff_centiseconds: WcaResult::new(6000),
            cutoff_attempts: 2,
        }),
    );

    if let ScorecardItem::Scorecard(sc) = &item {
        assert_eq!(sc.competitor.display_name(), ("Alice Smith", None));
        assert_eq!(sc.competitor.display_wca_id(), "2022SMIT01");
        assert_eq!(sc.truncated_competition_name(20), "Very Long Competi...");
        assert_eq!(
            sc.formatted_time_limit_info(),
            Some("Cutoff: < 1:00.00 (2 att)  |  Time limit: 5:00.00".to_string())
        );
    } else {
        panic!("Expected Scorecard");
    }
}

#[test]
fn test_time_limit_info_from_wcif_and_format() {
    let tl = TimeLimit {
        centiseconds: 60000,
        cumulative_round_ids: None,
    };
    let cutoff = Cutoff {
        number_of_attempts: 2,
        attempt_result: 4500,
    };

    let info = TimeLimitInfo::from_wcif(Some(&tl), Some(&cutoff)).unwrap();
    assert_eq!(info.limit_centiseconds, WcaResult::new(60000));
    assert_eq!(info.cutoff_centiseconds, WcaResult::new(4500));
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
fn test_wca_result_display_and_helpers() {
    assert_eq!(format!("{}", WcaResult::new(60000).unwrap()), "10:00.00");
    assert_eq!(format!("{}", WcaResult::new(9050).unwrap()), "1:30.50");
    assert_eq!(format!("{}", WcaResult::new(4500).unwrap()), "45.00");
    assert_eq!(format!("{}", WcaResult::new(805).unwrap()), "8.05");
    assert_eq!(format!("{}", WcaResult::DNF), "DNF");
    assert_eq!(format!("{}", WcaResult::DNS), "DNS");
    assert_eq!(format!("{}", WcaResult::new(0).unwrap()), "None");

    // Values < -2 are rejected by new()
    assert_eq!(WcaResult::new(-3), None);
    assert_eq!(WcaResult::new(-5), None);
    assert_eq!(WcaResult::new(-100), None);
    assert!(WcaResult::new(-2).is_some());
    assert!(WcaResult::new(-1).is_some());

    // try_new and TryFrom validation
    assert_eq!(
        WcaResult::try_new(60000),
        Ok(WcaResult::new(60000).unwrap())
    );
    assert_eq!(WcaResult::try_new(-2), Ok(WcaResult::DNS));
    assert_eq!(WcaResult::try_new(-1), Ok(WcaResult::DNF));
    assert_eq!(WcaResult::try_new(-3), Err(InvalidWcaResult(-3)));
    assert_eq!(WcaResult::try_from(-5), Err(InvalidWcaResult(-5)));
    assert_eq!(
        InvalidWcaResult(-5).to_string(),
        "invalid WCA result: -5 centiseconds (values < -2 are not allowed)"
    );

    let res = WcaResult::new(4500).unwrap();
    assert_eq!(res.centiseconds(), 4500);
    assert!(*res == 4500); // Deref<Target = i32>
    assert_eq!(res, 4500); // PartialEq<i32>
    assert_eq!(4500, res); // PartialEq<WcaResult> for i32
    assert!(res < 5000); // PartialOrd<i32>
    assert!(5000 > res); // PartialOrd<WcaResult> for i32
    assert!(res.is_valid_time());
    assert!(!res.is_dnf());
    assert!(!res.is_dns());

    assert!(WcaResult::DNF.is_dnf());
    assert!(!WcaResult::DNF.is_valid_time());
    assert!(WcaResult::DNS.is_dns());
    assert!(!WcaResult::DNS.is_valid_time());

    assert_eq!(WcaResult::from_centiseconds(4500), WcaResult::new(4500));
    assert_eq!(WcaResult::from_centiseconds(0), None);
    assert_eq!(WcaResult::from_centiseconds(-1), None);

    assert_eq!(
        TimeLimitInfo::format_centiseconds(9050),
        Some("1:30.50".to_string())
    );
    assert_eq!(TimeLimitInfo::format_centiseconds(0), None);
    assert_eq!(TimeLimitInfo::format_centiseconds(-1), None);
    assert_eq!(TimeLimitInfo::format_centiseconds(-3), None);
}

#[test]
fn test_scorecard_plan_summary_formatting() {
    let plan = ScorecardPlan {
        items: vec![],
        summaries: vec![
            PlannedRoundSummary::OpenRound {
                event: WcaEvent::E333,
                round_number: 1,
                competitor_count: 2,
                sample_competitor_names: vec!["Alice".to_string(), "Bob".to_string()],
            },
            PlannedRoundSummary::SubsequentRound {
                event: WcaEvent::E333,
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
    // When print_one_name is false and local_names_first is false: preserves original full name
    assert_eq!(
        planner::format_competitor_name("Zhang San (张三)", false, false),
        ("Zhang San", Some("张三"))
    );
    assert_eq!(
        planner::format_competitor_name("Lucas Burliga (Łukasz Burliga)", false, false),
        ("Lucas Burliga", Some("Łukasz Burliga"))
    );
    assert_eq!(
        planner::format_competitor_name("Alice Smith", false, false),
        ("Alice Smith", None)
    );

    // When print_one_name is true and local_names_first is false: strips parenthesized local or Latin name
    assert_eq!(
        planner::format_competitor_name("Zhang San (张三)", true, false),
        ("Zhang San", None)
    );
    assert_eq!(
        planner::format_competitor_name("Lucas Burliga (Łukasz Burliga)", true, false),
        ("Lucas Burliga", None)
    );
    assert_eq!(
        planner::format_competitor_name("Alice Smith", true, false),
        ("Alice Smith", None)
    );
    assert_eq!(
        planner::format_competitor_name("Kim Min-jun (김민준)", true, false),
        ("Kim Min-jun", None)
    );

    // When local_names_first is true: swaps primary name and parenthesized local name
    assert_eq!(
        planner::format_competitor_name("Zhang San (张三)", false, true),
        ("张三", Some("Zhang San"))
    );
    assert_eq!(
        planner::format_competitor_name("Lucas Burliga (Łukasz Burliga)", false, true),
        ("Łukasz Burliga", Some("Lucas Burliga"))
    );
    assert_eq!(
        planner::format_competitor_name("Alice Smith", false, true),
        ("Alice Smith", None)
    );

    // When print_one_name is true, the local name is omitted regardless of local_names_first
    assert_eq!(
        planner::format_competitor_name("Zhang San (张三)", true, true),
        ("Zhang San", None)
    );
    assert_eq!(
        planner::format_competitor_name("Lucas Burliga (Łukasz Burliga)", true, true),
        ("Lucas Burliga", None)
    );
    assert_eq!(
        planner::format_competitor_name("Alice Smith", true, true),
        ("Alice Smith", None)
    );
}
