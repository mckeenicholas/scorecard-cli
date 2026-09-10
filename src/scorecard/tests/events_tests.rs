use crate::scorecard::events::{self, ActivityCode, WcaEvent};

#[test]
fn test_event_name_by_id() {
    assert_eq!(events::event_name_by_id("333"), Some("3x3x3 Cube"));
    assert_eq!(events::event_name_by_id("222"), Some("2x2x2 Cube"));
    assert_eq!(events::event_name_by_id("333bf"), Some("3x3x3 Blindfolded"));
    assert_eq!(events::event_name_by_id("sq1"), Some("Square-1"));
    assert_eq!(events::event_name_by_id("invalid_event"), None);
}

#[test]
fn test_wca_event_properties() {
    assert_eq!(WcaEvent::ALL.len(), 17);
    for event in WcaEvent::ALL {
        let code = event.code();
        let name = event.display_name();
        assert!(!code.is_empty());
        assert!(!name.is_empty());
        assert_eq!(WcaEvent::from_id(code), Some(event));
        assert_eq!(code.parse::<WcaEvent>().unwrap(), event);
        assert_eq!(event.to_string(), code);
        assert_eq!(event.as_ref(), code);
    }
    assert!("invalid".parse::<WcaEvent>().is_err());
}

#[test]
fn test_activity_code_parsing_and_formatting() {
    // Round only
    let ac_round = ActivityCode::parse("333-r1").unwrap();
    assert_eq!(ac_round.event, WcaEvent::E333);
    assert_eq!(ac_round.round_number, 1);
    assert_eq!(ac_round.group_number, None);
    assert_eq!(ac_round.group_or_default(), 1);
    assert_eq!(ac_round.to_string(), "333-r1");
    assert_eq!(ac_round.round_id(), "333-r1");
    assert!(ac_round.matches_round(WcaEvent::E333, 1));
    assert!(!ac_round.matches_round(WcaEvent::E333, 2));
    assert!(!ac_round.matches_round(WcaEvent::E222, 1));

    // Round and group
    let ac_group = ActivityCode::parse("minx-r2-g3").unwrap();
    assert_eq!(ac_group.event, WcaEvent::Minx);
    assert_eq!(ac_group.round_number, 2);
    assert_eq!(ac_group.group_number, Some(3));
    assert_eq!(ac_group.group_or_default(), 3);
    assert_eq!(ac_group.to_string(), "minx-r2-g3");
    assert_eq!(ac_group.round_id(), "minx-r2");

    // Round constructors
    let round_ctor = ActivityCode::round(WcaEvent::Clock, 3);
    assert_eq!(round_ctor.to_string(), "clock-r3");
    let group_ctor = ActivityCode::group(WcaEvent::Sq1, 1, 2);
    assert_eq!(group_ctor.to_string(), "sq1-r1-g2");

    // Parse invalid cases
    assert!(ActivityCode::parse("other-lunch").is_none());
    assert!(ActivityCode::parse("333").is_none());
    assert!(ActivityCode::parse("333-1").is_none());
    assert!(ActivityCode::parse("333-r0").is_none());
    assert!(ActivityCode::parse("333-r1-g0").is_none());
    assert!(ActivityCode::parse("333-r1-g1-extra").is_none());
    assert!(ActivityCode::parse("invalid-r1").is_none());

    // FromStr
    assert_eq!(
        "444-r1-g2".parse::<ActivityCode>().unwrap(),
        ActivityCode::group(WcaEvent::E444, 1, 2)
    );
    assert!("invalid".parse::<ActivityCode>().is_err());
}
