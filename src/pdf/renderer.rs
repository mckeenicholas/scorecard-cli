use crate::scorecard::ScorecardItem;
use printpdf::color::{Color, Greyscale};
use printpdf::font::BuiltinFont;
use printpdf::graphics::{Line, LinePoint, PaintMode, Point, Rect};
use printpdf::ops::{Op, PdfFontHandle};
use printpdf::text::TextItem;
use printpdf::units::Pt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
}

pub struct TableSpec<'a> {
    pub tbl_x: f32,
    pub tbl_w: f32,
    pub col_widths: &'a [f32],
    pub headers: &'a [&'a str],
    pub rows: &'a [&'a [&'a str]],
    pub alignments: &'a [TextAlign],
    pub header_h: f32,
    pub row_h: f32,
}

pub struct TextSpec<'a> {
    pub text: &'a str,
    pub cell_x: f32,
    pub baseline_y: f32,
    pub cell_w: f32,
    pub font_size: f32,
    pub bold: bool,
    pub align: TextAlign,
}

// Pre-computed attempt log rows for standard WCA formats (zero runtime heap allocations)
macro_rules! make_attempt_table {
    ($($num:literal),*) => {
        [
            $(
                &[$num, "", "", "", ""],
            )*
    &["Extra", "", "", "", ""],
        ]
    };
}

const ATTEMPT_ROWS_5: [&[&str]; 6] = make_attempt_table!("1", "2", "3", "4", "5");
const ATTEMPT_ROWS_3: [&[&str]; 4] = make_attempt_table!("1", "2", "3");
const ATTEMPT_ROWS_2: [&[&str]; 3] = make_attempt_table!("1", "2");
const ATTEMPT_ROWS_1: [&[&str]; 2] = make_attempt_table!("1");

/// Renderer for drawing scorecard elements and vector primitives to a PDF page instruction stream.
pub struct ScorecardRenderer;

impl ScorecardRenderer {
    /// Draws a complete scorecard within the given bounding rectangle (x, y, w, h).
    pub fn draw_card(ops: &mut Vec<Op>, card: &ScorecardItem<'_>, x: f32, y: f32, w: f32, h: f32) {
        let top_y = y + h;
        let pad = 7.0;
        let inner_x = x + pad;
        let inner_w = w - 2.0 * pad;

        // 1. Outer scorecard boundary box
        ops.push(Op::SetOutlineColor {
            col: Color::Greyscale(Greyscale::new(0.0, None)),
        });
        ops.push(Op::SetOutlineThickness { pt: Pt(0.75) });
        ops.push(Op::DrawRectangle {
            rectangle: Rect {
                x: Pt(x),
                y: Pt(y),
                width: Pt(w),
                height: Pt(h),
                mode: Some(PaintMode::Stroke),
                winding_order: None,
            },
        });

        let mut cur_y = top_y - pad;

        // 2. Competition title header
        let comp_name = card.truncated_competition_name(30);
        cur_y -= 10.0;
        TextDrawer::draw(
            ops,
            TextSpec {
                text: &comp_name,
                cell_x: inner_x,
                baseline_y: cur_y,
                cell_w: inner_w,
                font_size: 9.5,
                bold: true,
                align: TextAlign::Center,
            },
        );

        // 3. Scorecard title header (e.g. SCORECARD #1)
        cur_y -= 13.0;
        let mut card_num_buf = itoa::Buffer::new();
        let card_title = if card.scorecard_number == 0 {
            "SCORECARD".to_string()
        } else {
            let num_str = card_num_buf.format(card.scorecard_number);
            let mut s = String::with_capacity(11 + num_str.len());
            s.push_str("SCORECARD #");
            s.push_str(num_str);
            s
        };

        TextDrawer::draw(
            ops,
            TextSpec {
                text: &card_title,
                cell_x: inner_x,
                baseline_y: cur_y,
                cell_w: inner_w,
                font_size: 11.0,
                bold: true,
                align: TextAlign::Center,
            },
        );

        // 4. Table 1: Event / Round / Group / Station
        cur_y -= 7.0;
        let mut station_buf = itoa::Buffer::new();
        let station_val = card
            .station_number
            .map(|s| station_buf.format(s))
            .unwrap_or("-");

        let mut round_buf = itoa::Buffer::new();
        let round_str = round_buf.format(card.round_number);

        let mut group_buf = itoa::Buffer::new();
        let group_str = group_buf.format(card.group_number);

        let t1_col_widths = [
            inner_w * 0.38,
            inner_w * 0.20,
            inner_w * 0.20,
            inner_w * 0.22,
        ];
        let t1_headers = ["Event", "Round", "Group", "Station"];
        let t1_row: [&str; 4] = [card.event_name, round_str, group_str, station_val];
        let t1_rows: [&[&str]; 1] = [&t1_row];
        let t1_align = [
            TextAlign::Left,
            TextAlign::Center,
            TextAlign::Center,
            TextAlign::Center,
        ];
        TableDrawer::draw(
            ops,
            &mut cur_y,
            TableSpec {
                tbl_x: inner_x,
                tbl_w: inner_w,
                col_widths: &t1_col_widths,
                headers: &t1_headers,
                rows: &t1_rows,
                alignments: &t1_align,
                header_h: 12.0,
                row_h: 13.0,
            },
        );

        // 5. Table 2: ID / Competitor Name
        cur_y -= 5.0;
        let mut id_buf = itoa::Buffer::new();
        let id_val = card
            .registrant_id
            .map(|id| id_buf.format(id))
            .unwrap_or("-");
        let name_val = card.display_competitor_name();
        let t2_col_widths = [inner_w * 0.20, inner_w * 0.80];
        let t2_headers = ["ID", "Competitor Name"];
        let t2_row: [&str; 2] = [id_val, &name_val];
        let t2_rows: [&[&str]; 1] = [&t2_row];
        let t2_align = [TextAlign::Center, TextAlign::Left];
        TableDrawer::draw(
            ops,
            &mut cur_y,
            TableSpec {
                tbl_x: inner_x,
                tbl_w: inner_w,
                col_widths: &t2_col_widths,
                headers: &t2_headers,
                rows: &t2_rows,
                alignments: &t2_align,
                header_h: 12.0,
                row_h: 14.0,
            },
        );

        // 6. Table 3: Attempt Log
        cur_y -= 5.0;
        let t3_col_widths = [
            inner_w * 0.16,
            inner_w * 0.15,
            inner_w * 0.39,
            inner_w * 0.15,
            inner_w * 0.15,
        ];
        let t3_headers = ["Attempt", "Scr", "Result", "Judge", "Comp"];
        let t3_align = [
            TextAlign::Center,
            TextAlign::Center,
            TextAlign::Center,
            TextAlign::Center,
            TextAlign::Center,
        ];

        let header_h = 12.0;

        match card.attempt_count {
            5 => {
                let available_attempt_h = cur_y - (y + pad);
                let t3_row_h = ((available_attempt_h - header_h) / 6.0).clamp(14.0, 18.5);
                TableDrawer::draw(
                    ops,
                    &mut cur_y,
                    TableSpec {
                        tbl_x: inner_x,
                        tbl_w: inner_w,
                        col_widths: &t3_col_widths,
                        headers: &t3_headers,
                        rows: &ATTEMPT_ROWS_5,
                        alignments: &t3_align,
                        header_h,
                        row_h: t3_row_h,
                    },
                );
            }
            3 => {
                let available_attempt_h = cur_y - (y + pad);
                let t3_row_h = ((available_attempt_h - header_h) / 4.0).clamp(14.0, 18.5);
                TableDrawer::draw(
                    ops,
                    &mut cur_y,
                    TableSpec {
                        tbl_x: inner_x,
                        tbl_w: inner_w,
                        col_widths: &t3_col_widths,
                        headers: &t3_headers,
                        rows: &ATTEMPT_ROWS_3,
                        alignments: &t3_align,
                        header_h,
                        row_h: t3_row_h,
                    },
                );
            }
            2 => {
                let available_attempt_h = cur_y - (y + pad);
                let t3_row_h = ((available_attempt_h - header_h) / 3.0).clamp(14.0, 18.5);
                TableDrawer::draw(
                    ops,
                    &mut cur_y,
                    TableSpec {
                        tbl_x: inner_x,
                        tbl_w: inner_w,
                        col_widths: &t3_col_widths,
                        headers: &t3_headers,
                        rows: &ATTEMPT_ROWS_2,
                        alignments: &t3_align,
                        header_h,
                        row_h: t3_row_h,
                    },
                );
            }
            1 => {
                let available_attempt_h = cur_y - (y + pad);
                let t3_row_h = ((available_attempt_h - header_h) / 2.0).clamp(14.0, 18.5);
                TableDrawer::draw(
                    ops,
                    &mut cur_y,
                    TableSpec {
                        tbl_x: inner_x,
                        tbl_w: inner_w,
                        col_widths: &t3_col_widths,
                        headers: &t3_headers,
                        rows: &ATTEMPT_ROWS_1,
                        alignments: &t3_align,
                        header_h,
                        row_h: t3_row_h,
                    },
                );
            }
            count => {
                let attempt_str_pool: Vec<String> = (1..=count).map(|i| i.to_string()).collect();
                let mut dynamic_row_storage: Vec<[&str; 5]> = Vec::with_capacity(count + 1);
                for s in &attempt_str_pool {
                    dynamic_row_storage.push([s.as_str(), "", "", "", ""]);
                }
                dynamic_row_storage.push(["Extra", "", "", "", ""]);

                let dynamic_rows: Vec<&[&str]> =
                    dynamic_row_storage.iter().map(|r| r.as_slice()).collect();

                let available_attempt_h = cur_y - (y + pad);
                let row_count = dynamic_rows.len() as f32;
                let t3_row_h = ((available_attempt_h - header_h) / row_count).clamp(14.0, 18.5);

                TableDrawer::draw(
                    ops,
                    &mut cur_y,
                    TableSpec {
                        tbl_x: inner_x,
                        tbl_w: inner_w,
                        col_widths: &t3_col_widths,
                        headers: &t3_headers,
                        rows: &dynamic_rows,
                        alignments: &t3_align,
                        header_h,
                        row_h: t3_row_h,
                    },
                );
            }
        }
    }
}

/// Helper struct for drawing grid tables with shaded headers and borders.
pub struct TableDrawer;

impl TableDrawer {
    pub fn draw(ops: &mut Vec<Op>, cur_y: &mut f32, spec: TableSpec<'_>) {
        let top_y = *cur_y;
        let total_h = spec.header_h + spec.row_h * (spec.rows.len() as f32);
        let bottom_y = top_y - total_h;

        // 1. Header background fill
        ops.push(Op::SetFillColor {
            col: Color::Greyscale(Greyscale::new(0.92, None)),
        });
        ops.push(Op::DrawRectangle {
            rectangle: Rect {
                x: Pt(spec.tbl_x),
                y: Pt(top_y - spec.header_h),
                width: Pt(spec.tbl_w),
                height: Pt(spec.header_h),
                mode: Some(PaintMode::Fill),
                winding_order: None,
            },
        });

        // 2. Header text
        let mut col_x = spec.tbl_x;
        for (i, &header) in spec.headers.iter().enumerate() {
            let w = spec.col_widths[i];
            let text_y = top_y - spec.header_h + (spec.header_h - 7.5) / 2.0 + 1.0;
            let align = spec.alignments.get(i).copied().unwrap_or(TextAlign::Center);
            TextDrawer::draw(
                ops,
                TextSpec {
                    text: header,
                    cell_x: col_x,
                    baseline_y: text_y,
                    cell_w: w,
                    font_size: 7.5,
                    bold: true,
                    align,
                },
            );
            col_x += w;
        }

        // 3. Row data text
        let mut row_top = top_y - spec.header_h;
        for &row in spec.rows {
            let mut cell_x = spec.tbl_x;
            let text_y = row_top - spec.row_h + (spec.row_h - 8.0) / 2.0 + 1.0;
            for (i, &cell) in row.iter().enumerate() {
                let w = spec.col_widths[i];
                let align = spec.alignments.get(i).copied().unwrap_or(TextAlign::Center);
                TextDrawer::draw(
                    ops,
                    TextSpec {
                        text: cell,
                        cell_x,
                        baseline_y: text_y,
                        cell_w: w,
                        font_size: 8.0,
                        bold: false,
                        align,
                    },
                );
                cell_x += w;
            }
            row_top -= spec.row_h;
        }

        // 4. Grid lines (Borders)
        ops.push(Op::SetOutlineColor {
            col: Color::Greyscale(Greyscale::new(0.55, None)),
        });
        ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });

        // Outer table border
        ops.push(Op::DrawRectangle {
            rectangle: Rect {
                x: Pt(spec.tbl_x),
                y: Pt(bottom_y),
                width: Pt(spec.tbl_w),
                height: Pt(total_h),
                mode: Some(PaintMode::Stroke),
                winding_order: None,
            },
        });

        // Horizontal line after header
        Self::draw_line(
            ops,
            spec.tbl_x,
            top_y - spec.header_h,
            spec.tbl_x + spec.tbl_w,
            top_y - spec.header_h,
        );

        // Horizontal lines between rows
        let mut line_y = top_y - spec.header_h - spec.row_h;
        for _ in 1..spec.rows.len() {
            Self::draw_line(ops, spec.tbl_x, line_y, spec.tbl_x + spec.tbl_w, line_y);
            line_y -= spec.row_h;
        }

        // Vertical column separator lines
        let mut sep_x = spec.tbl_x;
        for &w in &spec.col_widths[..spec.col_widths.len() - 1] {
            sep_x += w;
            Self::draw_line(ops, sep_x, top_y, sep_x, bottom_y);
        }

        *cur_y = bottom_y;
    }

    fn draw_line(ops: &mut Vec<Op>, x1: f32, y1: f32, x2: f32, y2: f32) {
        ops.push(Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point {
                            x: Pt(x1),
                            y: Pt(y1),
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: Pt(x2),
                            y: Pt(y2),
                        },
                        bezier: false,
                    },
                ],
                is_closed: false,
            },
        });
    }
}

/// Helper struct for rendering text with Helvetica proportional font metrics and horizontal alignment.
pub struct TextDrawer;

impl TextDrawer {
    pub fn draw(ops: &mut Vec<Op>, spec: TextSpec<'_>) {
        if spec.text.is_empty() {
            return;
        }
        let font = if spec.bold {
            PdfFontHandle::Builtin(BuiltinFont::HelveticaBold)
        } else {
            PdfFontHandle::Builtin(BuiltinFont::Helvetica)
        };

        let text_w = Self::estimate_width(spec.text, spec.font_size);
        let pad = 3.0;
        let x = match spec.align {
            TextAlign::Left => spec.cell_x + pad,
            TextAlign::Center => spec.cell_x + (spec.cell_w - text_w).max(0.0) / 2.0,
        };

        ops.push(Op::SetFillColor {
            col: Color::Greyscale(Greyscale::new(0.0, None)),
        });
        ops.push(Op::StartTextSection);
        ops.push(Op::SetFont {
            font,
            size: Pt(spec.font_size),
        });
        ops.push(Op::SetTextCursor {
            pos: Point {
                x: Pt(x),
                y: Pt(spec.baseline_y),
            },
        });
        ops.push(Op::ShowText {
            items: vec![TextItem::Text(spec.text.to_string())],
        });
        ops.push(Op::EndTextSection);
    }

    /// Approximates proportional Helvetica font character widths for centering.
    pub fn estimate_width(text: &str, font_size: f32) -> f32 {
        let mut w = 0.0;
        for ch in text.chars() {
            let factor = match ch {
                ' ' => 0.28,
                '.' | ',' | ':' | ';' | '!' | '|' | '\'' | '`' | 'i' | 'j' | 'l' | 'I' => 0.26,
                'f' | 't' | '(' | ')' | '[' | ']' | '{' | '}' => 0.32,
                'r' => 0.36,
                'm' | 'w' | 'M' | 'W' => 0.78,
                'A'..='Z' => 0.62,
                '0'..='9' => 0.55,
                _ => 0.50,
            };
            w += factor * font_size;
        }
        w
    }
}
