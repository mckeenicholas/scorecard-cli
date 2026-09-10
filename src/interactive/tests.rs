use super::suggest;
use super::types::{CoverSheetChoice, ExtraOption};

#[test]
fn test_local_json_suggestions() {
    let suggestions = suggest::get_local_json_suggestions("test_wcif");
    assert!(
        suggestions
            .iter()
            .any(|s| s.value.contains("tests/test_wcif.json")
                || s.value.contains("test_wcif.json")),
        "Expected test_wcif.json to be found in local json suggestions"
    );

    let empty_query = suggest::get_local_json_suggestions("");
    assert!(
        !empty_query.is_empty(),
        "Expected empty query to return all local json files"
    );
}

#[test]
fn test_tilde_and_root_path_suggestions() {
    let root_suggestions = suggest::get_local_json_suggestions("/");
    assert!(
        !root_suggestions.is_empty(),
        "Expected root path suggestions to return directories"
    );
    assert!(
        root_suggestions.iter().any(|s| s.value.starts_with('/')),
        "Root suggestions should begin with /"
    );

    let tilde_suggestions = suggest::get_local_json_suggestions("~");
    assert!(
        !tilde_suggestions.is_empty(),
        "Expected home path suggestions to return entries"
    );
    assert!(
        tilde_suggestions.iter().any(|s| s.value.starts_with("~/")),
        "Tilde suggestions should begin with ~/"
    );

    let tilde_slash = suggest::get_local_json_suggestions("~/");
    assert!(
        !tilde_slash.is_empty(),
        "Expected ~/ suggestions to return entries"
    );
}

#[test]
fn test_cover_sheet_choice_display() {
    assert_eq!(
        CoverSheetChoice::Stage.to_string(),
        "By Stage (one per group on each stage)"
    );
    assert_eq!(
        CoverSheetChoice::Group.to_string(),
        "By Group (one per group across all stages)"
    );
    assert_eq!(
        CoverSheetChoice::Round.to_string(),
        "By Round (one for the entire round)"
    );
}

#[test]
fn test_extra_option_display() {
    assert!(
        ExtraOption::StartGroupOnNewPage
            .to_string()
            .contains("Start group on new page")
    );
    assert_eq!(
        ExtraOption::PrintStations.to_string(),
        "Print station numbers"
    );
    assert_eq!(ExtraOption::PrintOneName.to_string(), "Only print one name");
}

#[test]
fn test_format_wca_suggestion_alignment() {
    let s1 =
        suggest::format_wca_suggestion("NAC2026", "North American Championship", Some("US"), 22);
    let s2 =
        suggest::format_wca_suggestion("AjaxAutumnAM2026", "Ajax Autumn AM 2026", Some("CA"), 22);
    assert_eq!(
        s1,
        "NAC2026                 North American Championship (US)"
    );
    assert_eq!(s2, "AjaxAutumnAM2026        Ajax Autumn AM 2026 (CA)");
    // Column 2 starts at index 24 (22 chars for ID + 2 spaces)
    assert_eq!(&s1[..24], "NAC2026                 ");
    assert_eq!(&s2[..24], "AjaxAutumnAM2026        ");
}
