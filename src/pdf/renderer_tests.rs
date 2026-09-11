use printpdf::FontId;
use printpdf::font::BuiltinFont;
use printpdf::ops::Op;

use crate::pdf::layout::RectSpec;
use crate::pdf::renderer::ScorecardRenderer;
use crate::pdf::text::{CompetitorNameSpec, TextDrawer};
use crate::pdf::theme;
use crate::scorecard::{
    BlankScorecard, Competitor, CoverSheet, Scorecard, ScorecardItem, TimeLimitInfo, WcaEvent,
    WcaId, WcaResult,
};

#[test]
fn test_estimate_width() {
    let w_space = TextDrawer::estimate_width(" ", 10.0, false);
    assert!((w_space - 2.78).abs() < 0.01);

    let w_digits = TextDrawer::estimate_width("12345", 10.0, false);
    assert!((w_digits - 27.8).abs() < 0.01);

    let w_empty = TextDrawer::estimate_width("", 10.0, false);
    assert_eq!(w_empty, 0.0);

    let w_cjk_name = TextDrawer::estimate_competitor_name_width("", Some("张"), 10.0);
    // "(" (3.33) + ")" (3.33) + "张" (10.0) + 2 * (0.10 * 10.0) (2.0) = 18.66
    assert!((w_cjk_name - 18.66).abs() < 0.05);
}

#[test]
fn test_draw_card_operations() {
    let card = Scorecard::new(
        "Test Comp 2026",
        WcaEvent::E333,
        1,
        1,
        Competitor {
            name: "Alice Smith",
            local_name: None,
            registrant_id: std::num::NonZeroUsize::MIN,
            wca_id: WcaId::parse("2022SMIT01"),
        },
    )
    .with_stage(Some("Red Stage"))
    .with_station(Some(4))
    .with_time_limit(Some(TimeLimitInfo {
        limit_centiseconds: WcaResult::new(60000),
        is_cumulative: false,
        cutoff_centiseconds: None,
        cutoff_attempts: 0,
    }))
    .into();

    let mut ops = Vec::new();
    ScorecardRenderer::draw_card(
        &mut ops,
        &card,
        RectSpec::new(18.0, 18.0, 270.0, 380.0),
        None,
    );

    // Verify that operations were generated (borders, rects, text items)
    assert!(!ops.is_empty());
    let has_rectangles = ops.iter().any(|op| matches!(op, Op::DrawRectangle { .. }));
    let has_text = ops.iter().any(|op| matches!(op, Op::ShowText { .. }));
    assert!(has_rectangles);
    assert!(has_text);

    // Verify competitor name is drawn in bold font
    let has_bold_name = ops.windows(3).any(|window| {
        let is_bold_font = matches!(&window[0], Op::SetFont { font: printpdf::PdfFontHandle::Builtin(printpdf::BuiltinFont::HelveticaBold), .. });
        let has_alice = matches!(&window[2], Op::ShowText { items } if items.iter().any(|it| match it {
            printpdf::ops::TextItem::Text(s) => s == "Alice Smith",
            _ => false,
        }));
        is_bold_font && has_alice
    });
    assert!(
        has_bold_name,
        "Competitor name should be rendered in bold font"
    );
}

#[test]
fn test_draw_empty_space_produces_no_ops() {
    let empty = ScorecardItem::empty_space();
    let mut ops = Vec::new();
    ScorecardRenderer::draw_card(
        &mut ops,
        &empty,
        RectSpec::new(0.0, 0.0, 100.0, 100.0),
        None,
    );
    assert!(
        ops.is_empty(),
        "Empty space item should generate zero drawing operations"
    );
}

#[test]
fn test_draw_cover_sheet_operations() {
    let cover_card = CoverSheet::new("Ocean State Cubikon 2025", WcaEvent::E333, 1, 1, 15)
        .with_stage(Some("Main Hall"))
        .into();

    let mut ops = Vec::new();
    ScorecardRenderer::draw_card(
        &mut ops,
        &cover_card,
        RectSpec::new(18.0, 18.0, 270.0, 380.0),
        None,
    );

    assert!(!ops.is_empty());
    let has_rectangles = ops.iter().any(|op| matches!(op, Op::DrawRectangle { .. }));
    let has_text = ops.iter().any(|op| matches!(op, Op::ShowText { .. }));
    let has_lines = ops.iter().any(|op| matches!(op, Op::DrawLine { .. }));
    assert!(has_rectangles);
    assert!(has_text);
    assert!(has_lines);
}

#[test]
fn test_theme_defaults() {
    let theme = theme::DEFAULT_THEME;
    assert_eq!(theme.padding, 7.0);
    assert_eq!(theme.border_thickness, 0.75);
    assert!(theme.title_font_size > theme.header_font_size);
}

#[test]
fn test_event_info_table_3_cols_without_station() {
    let card_no_station = Scorecard::new(
        "Test Comp 2026",
        WcaEvent::E333,
        1,
        1,
        Competitor::simple("Alice Smith"),
    )
    .into();

    let mut ops_no_station = Vec::new();
    ScorecardRenderer::draw_card(
        &mut ops_no_station,
        &card_no_station,
        RectSpec::new(18.0, 18.0, 270.0, 380.0),
        None,
    );
    let has_station_header = ops_no_station.iter().any(|op| match op {
        Op::ShowText { items } => items.iter().any(|item| match item {
            printpdf::ops::TextItem::Text(s) => s.as_str() == "Station",
            _ => false,
        }),
        _ => false,
    });
    assert!(
        !has_station_header,
        "Expected no 'Station' header when station_number is None"
    );

    let card_with_station = Scorecard::new(
        "Test Comp 2026",
        WcaEvent::E333,
        1,
        1,
        Competitor::simple("Alice Smith"),
    )
    .with_station(Some(3))
    .into();
    let mut ops_with_station = Vec::new();
    ScorecardRenderer::draw_card(
        &mut ops_with_station,
        &card_with_station,
        RectSpec::new(18.0, 18.0, 270.0, 380.0),
        None,
    );
    let has_station_header_with = ops_with_station.iter().any(|op| match op {
        Op::ShowText { items } => items.iter().any(|item| match item {
            printpdf::ops::TextItem::Text(s) => s.as_str() == "Station",
            _ => false,
        }),
        _ => false,
    });
    assert!(
        has_station_header_with,
        "Expected 'Station' header when station_number is Some"
    );
}

#[test]
fn test_attempt_table_cutoff_and_extra_banners() {
    let card = Scorecard::new(
        "Test Comp 2026",
        WcaEvent::E333,
        1,
        1,
        Competitor::simple("Alice Smith"),
    )
    .with_time_limit(Some(TimeLimitInfo {
        limit_centiseconds: WcaResult::new(60000),
        is_cumulative: false,
        cutoff_centiseconds: WcaResult::new(4500),
        cutoff_attempts: 2,
    }))
    .into();

    let mut ops = Vec::new();
    ScorecardRenderer::draw_card(
        &mut ops,
        &card,
        RectSpec::new(18.0, 18.0, 270.0, 380.0),
        None,
    );

    // Verify that the cutoff banner text is rendered
    let has_cutoff_banner = ops.iter().any(|op| match op {
        Op::ShowText { items } => items.iter().any(|item| match item {
            printpdf::ops::TextItem::Text(s) => {
                s.contains("Must have solve under 45.00 to complete average")
            }
            _ => false,
        }),
        _ => false,
    });
    assert!(
        has_cutoff_banner,
        "Cutoff banner text should be rendered between solves"
    );

    // Verify that the extra solve banner text is rendered
    let has_extra_banner = ops.iter().any(|op| match op {
        Op::ShowText { items } => items.iter().any(|item| match item {
            printpdf::ops::TextItem::Text(s) => s.contains("Extra or provisional solve"),
            _ => false,
        }),
        _ => false,
    });
    assert!(
        has_extra_banner,
        "Extra solve banner text should be rendered"
    );

    // Verify that the bottom time limit footer is rendered
    let has_time_limit_footer = ops.iter().any(|op| match op {
        Op::ShowText { items } => items.iter().any(|item| match item {
            printpdf::ops::TextItem::Text(s) => s == "Time limit: 10:00.00",
            _ => false,
        }),
        _ => false,
    });
    assert!(
        has_time_limit_footer,
        "Time limit footer should be rendered at the bottom"
    );
}

#[test]
fn test_draw_blank_card_omits_wca_id_and_id_hyphen() {
    let blank_card = BlankScorecard::new("Test Comp 2026", WcaEvent::E333, 2, 1).into();

    let mut ops = Vec::new();
    ScorecardRenderer::draw_card(
        &mut ops,
        &blank_card,
        RectSpec::new(18.0, 18.0, 270.0, 380.0),
        None,
    );

    let has_wca_id_header = ops.iter().any(|op| match op {
        Op::ShowText { items } => items.iter().any(|item| match item {
            printpdf::ops::TextItem::Text(s) => s.contains("WCA ID"),
            _ => false,
        }),
        _ => false,
    });
    assert!(
        !has_wca_id_header,
        "Blank scorecard must omit the WCA ID column header"
    );

    let has_hyphen = ops.iter().any(|op| match op {
        Op::ShowText { items } => items.iter().any(|item| match item {
            printpdf::ops::TextItem::Text(s) => s.as_str() == "-",
            _ => false,
        }),
        _ => false,
    });
    assert!(
        !has_hyphen,
        "Blank scorecard must not print '-' in the ID space"
    );
}

#[test]
fn test_mixed_font_text_runs() {
    let mut ops = Vec::new();
    let custom_font = FontId("CustomFontTest".to_owned());
    TextDrawer::draw_competitor_name(
        &mut ops,
        CompetitorNameSpec {
            primary: "Marco Yang",
            local: Some("杨柯辰"),
            start_x: 13.0,
            baseline_y: 100.0,
            font_size: 10.0,
            custom_font: Some(&custom_font),
        },
    );

    let text_runs: Vec<String> = ops
        .iter()
        .filter_map(|op| match op {
            Op::ShowText { items } => items.first().and_then(|item| match item {
                printpdf::ops::TextItem::Text(s) => Some(s.clone()),
                _ => None,
            }),
            _ => None,
        })
        .collect();

    assert_eq!(text_runs, vec!["Marco Yang", " (", "杨柯辰", ")"]);

    let font_handles: Vec<printpdf::ops::PdfFontHandle> = ops
        .iter()
        .filter_map(|op| match op {
            Op::SetFont { font, .. } => Some(font.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(
        font_handles,
        vec![
            printpdf::ops::PdfFontHandle::Builtin(BuiltinFont::HelveticaBold),
            printpdf::ops::PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            printpdf::ops::PdfFontHandle::External(FontId("CustomFontTest".to_owned())),
            printpdf::ops::PdfFontHandle::Builtin(BuiltinFont::Helvetica),
        ]
    );

    let cursor_x_positions: Vec<f32> = ops
        .iter()
        .filter_map(|op| match op {
            Op::SetTextCursor { pos } => Some(pos.x.0),
            _ => None,
        })
        .collect();

    assert_eq!(cursor_x_positions.len(), 4);
    let x_latin = cursor_x_positions[0];
    let x_open = cursor_x_positions[1];
    let x_cjk = cursor_x_positions[2];
    let x_close = cursor_x_positions[3];

    assert!((x_latin - 13.0).abs() < 1e-4);
    let latin_w = TextDrawer::estimate_width("Marco Yang", 10.0, true);
    assert!((x_open - (x_latin + latin_w)).abs() < 1e-4);
    let open_paren_w = TextDrawer::estimate_width(" (", 10.0, false);
    assert!((x_cjk - (x_open + open_paren_w + 1.0)).abs() < 1e-4);
    assert!((x_close - (x_cjk + 30.0 + 1.0)).abs() < 1e-4);
}

#[test]
fn test_mixed_font_text_runs_local_names_first() {
    let mut ops = Vec::new();
    let custom_font = FontId("CustomFontTest".to_owned());
    TextDrawer::draw_competitor_name(
        &mut ops,
        CompetitorNameSpec {
            primary: "杨柯辰",
            local: Some("Marco Yang"),
            start_x: 13.0,
            baseline_y: 100.0,
            font_size: 10.0,
            custom_font: Some(&custom_font),
        },
    );

    let text_runs: Vec<String> = ops
        .iter()
        .filter_map(|op| match op {
            Op::ShowText { items } => items.first().and_then(|item| match item {
                printpdf::ops::TextItem::Text(s) => Some(s.clone()),
                _ => None,
            }),
            _ => None,
        })
        .collect();

    assert_eq!(text_runs, vec!["杨柯辰", " (", "Marco Yang", ")"]);

    let font_handles: Vec<printpdf::ops::PdfFontHandle> = ops
        .iter()
        .filter_map(|op| match op {
            Op::SetFont { font, .. } => Some(font.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(
        font_handles,
        vec![
            printpdf::ops::PdfFontHandle::External(FontId("CustomFontTest".to_owned())),
            printpdf::ops::PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            printpdf::ops::PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            printpdf::ops::PdfFontHandle::Builtin(BuiltinFont::Helvetica),
        ]
    );

    let cursor_x_positions: Vec<f32> = ops
        .iter()
        .filter_map(|op| match op {
            Op::SetTextCursor { pos } => Some(pos.x.0),
            _ => None,
        })
        .collect();

    assert_eq!(cursor_x_positions.len(), 4);
    let x_cjk = cursor_x_positions[0];
    let x_open = cursor_x_positions[1];
    let x_latin = cursor_x_positions[2];
    let x_close = cursor_x_positions[3];

    assert!((x_cjk - 13.0).abs() < 1e-4);
    // CJK width is 3 * 10.0 = 30.0 pt
    assert!((x_open - (x_cjk + 30.0)).abs() < 1e-4);
    let open_paren_w = TextDrawer::estimate_width(" (", 10.0, false);
    assert!((x_latin - (x_open + open_paren_w)).abs() < 1e-4);
    let latin_w = TextDrawer::estimate_width("Marco Yang", 10.0, false);
    assert!((x_close - (x_latin + latin_w)).abs() < 1e-4);
}

#[test]
fn test_draw_card_with_scramble_checker_renders_check_column() {
    let card = Scorecard::new(
        "Test Comp 2026",
        WcaEvent::E333,
        1,
        1,
        Competitor::simple("Alice Smith"),
    )
    .with_scramble_checker(true)
    .into();

    let mut ops = Vec::new();
    ScorecardRenderer::draw_card(
        &mut ops,
        &card,
        RectSpec::new(18.0, 18.0, 270.0, 380.0),
        None,
    );

    let has_check_header = ops.iter().any(|op| match op {
        Op::ShowText { items } => items.iter().any(|item| match item {
            printpdf::ops::TextItem::Text(s) => s.as_str() == "Check",
            _ => false,
        }),
        _ => false,
    });
    assert!(
        has_check_header,
        "Expected 'Check' header column when needs_scramble_checker is true"
    );
}

#[test]
fn test_draw_card_without_scramble_checker_omits_check_column() {
    let card = Scorecard::new(
        "Test Comp 2026",
        WcaEvent::E333,
        1,
        1,
        Competitor::simple("Alice Smith"),
    )
    .into();

    let mut ops = Vec::new();
    ScorecardRenderer::draw_card(
        &mut ops,
        &card,
        RectSpec::new(18.0, 18.0, 270.0, 380.0),
        None,
    );

    let has_check_header = ops.iter().any(|op| match op {
        Op::ShowText { items } => items.iter().any(|item| match item {
            printpdf::ops::TextItem::Text(s) => s.as_str() == "Check",
            _ => false,
        }),
        _ => false,
    });
    assert!(
        !has_check_header,
        "Expected no 'Check' header column when needs_scramble_checker is false"
    );
}
