use crate::scorecard::{ScorecardItem, TimeLimitInfo};
use printpdf::color::{Color, Greyscale};
use printpdf::font::BuiltinFont;
use printpdf::graphics::{Line, LinePoint, PaintMode, Point, Rect};
use printpdf::ops::{Op, PdfFontHandle};
use printpdf::text::TextItem;
use printpdf::units::Pt;

/// Text alignment within a scorecard cell or bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
}

/// Visual theme and geometric styling parameters for scorecard rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScorecardTheme {
    pub padding: f32,
    pub border_thickness: f32,
    pub header_bg_grey: f32,
    pub grid_line_grey: f32,
    pub grid_line_thickness: f32,
    pub title_font_size: f32,
    pub header_font_size: f32,
    pub cell_font_size: f32,
    pub comp_name_font_size: f32,
}

/// Default styling theme matching official WCA competition scorecard aesthetics.
pub const DEFAULT_THEME: ScorecardTheme = ScorecardTheme {
    padding: 7.0,
    border_thickness: 0.75,
    header_bg_grey: 0.92,
    grid_line_grey: 0.55,
    grid_line_thickness: 0.5,
    title_font_size: 11.0,
    header_font_size: 7.5,
    cell_font_size: 8.0,
    comp_name_font_size: 9.5,
};

/// Column specification for grid tables (header label, width ratio [0.0..1.0], text alignment).
pub type ColumnDef<'a> = (&'a str, f32, TextAlign);

/// Specification for rendering a structured grid table.
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

/// Specification for rendering text with alignment and font styling.
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

const ATTEMPT_COLUMNS: [ColumnDef<'static>; 5] = [
    ("Attempt", 0.16, TextAlign::Center),
    ("Scr", 0.15, TextAlign::Center),
    ("Result", 0.39, TextAlign::Center),
    ("Judge", 0.15, TextAlign::Center),
    ("Comp", 0.15, TextAlign::Center),
];

/// Canvas abstraction managing vertical flow, bounding geometry, and rendering primitives for a scorecard.
pub struct CardPainter<'a> {
    pub ops: &'a mut Vec<Op>,
    pub theme: &'a ScorecardTheme,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub inner_x: f32,
    pub inner_w: f32,
    pub cur_y: f32,
    pub min_y: f32,
}

impl<'a> CardPainter<'a> {
    pub fn new(
        ops: &'a mut Vec<Op>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        theme: &'a ScorecardTheme,
    ) -> Self {
        let pad = theme.padding;
        let inner_x = x + pad;
        let inner_w = w - 2.0 * pad;
        let top_y = y + h;
        let cur_y = top_y - pad;
        let min_y = y + pad;

        Self {
            ops,
            theme,
            x,
            y,
            w,
            h,
            inner_x,
            inner_w,
            cur_y,
            min_y,
        }
    }

    /// Advances the vertical cursor downward by `dy` points.
    #[inline]
    pub fn advance_y(&mut self, dy: f32) {
        self.cur_y -= dy;
    }

    /// Draws the outer scorecard bounding box border.
    pub fn draw_outer_border(&mut self) {
        self.ops.push(Op::SetOutlineColor {
            col: Color::Greyscale(Greyscale::new(0.0, None)),
        });
        self.ops.push(Op::SetOutlineThickness {
            pt: Pt(self.theme.border_thickness),
        });
        self.ops.push(Op::DrawRectangle {
            rectangle: Rect {
                x: Pt(self.x),
                y: Pt(self.y),
                width: Pt(self.w),
                height: Pt(self.h),
                mode: Some(PaintMode::Stroke),
                winding_order: None,
            },
        });
    }

    /// Draws the top header: scorecard number in top-left corner and competition name centered.
    pub fn draw_top_header(&mut self, scorecard_number: usize, comp_name: &str) {
        self.advance_y(10.0);

        if scorecard_number > 0 {
            let mut num_buf = itoa::Buffer::new();
            let num_str = num_buf.format(scorecard_number);
            TextDrawer::draw(
                self.ops,
                TextSpec {
                    text: num_str,
                    cell_x: self.inner_x,
                    baseline_y: self.cur_y,
                    cell_w: 40.0,
                    font_size: self.theme.title_font_size,
                    bold: true,
                    align: TextAlign::Left,
                },
            );
        }

        TextDrawer::draw(
            self.ops,
            TextSpec {
                text: comp_name,
                cell_x: self.inner_x,
                baseline_y: self.cur_y,
                cell_w: self.inner_w,
                font_size: self.theme.comp_name_font_size,
                bold: true,
                align: TextAlign::Center,
            },
        );

        self.advance_y(7.0);
    }

    /// Draws a structured grid table with specified column ratios, header height, and row height.
    pub fn draw_grid_table(
        &mut self,
        header_h: f32,
        row_h: f32,
        columns: &[ColumnDef<'_>],
        rows: &[&[&str]],
    ) {
        let mut col_widths = [0.0f32; 8];
        let mut headers = [""; 8];
        let mut alignments = [TextAlign::Center; 8];
        let col_count = columns.len().min(8);

        for (i, &(header, ratio, align)) in columns.iter().take(col_count).enumerate() {
            col_widths[i] = self.inner_w * ratio;
            headers[i] = header;
            alignments[i] = align;
        }

        TableDrawer::draw(
            self.ops,
            &mut self.cur_y,
            TableSpec {
                tbl_x: self.inner_x,
                tbl_w: self.inner_w,
                col_widths: &col_widths[..col_count],
                headers: &headers[..col_count],
                rows,
                alignments: &alignments[..col_count],
                header_h,
                row_h,
            },
            self.theme,
        );
    }

    /// Draws the event, round, group, and station info grid table.
    pub fn draw_event_info_table(&mut self, card: &ScorecardItem<'_>) {
        let mut station_buf = itoa::Buffer::new();
        let station_val = card
            .station_number
            .map(|s| station_buf.format(s))
            .unwrap_or("-");

        let mut round_buf = itoa::Buffer::new();
        let round_str = round_buf.format(card.round_number);

        let mut group_buf = itoa::Buffer::new();
        let group_str = group_buf.format(card.group_number);

        self.draw_grid_table(
            12.0,
            13.0,
            &[
                ("Event", 0.38, TextAlign::Left),
                ("Round", 0.20, TextAlign::Center),
                ("Group", 0.20, TextAlign::Center),
                ("Station", 0.22, TextAlign::Center),
            ],
            &[&[card.event_name, round_str, group_str, station_val]],
        );
    }

    /// Draws the competitor ID and name grid table.
    pub fn draw_competitor_info_table(&mut self, card: &ScorecardItem<'_>) {
        self.advance_y(5.0);
        let mut id_buf = itoa::Buffer::new();
        let id_val = card
            .registrant_id
            .map(|id| id_buf.format(id))
            .unwrap_or("-");
        let name_val = card.display_competitor_name();

        self.draw_grid_table(
            12.0,
            14.0,
            &[
                ("ID", 0.20, TextAlign::Center),
                ("Competitor Name", 0.80, TextAlign::Left),
            ],
            &[&[id_val, &name_val]],
        );
    }

    /// Draws the attempt table dynamically sized to fill the remaining scorecard height, plus bottom cutoff/time limit footer.
    pub fn draw_attempt_table(
        &mut self,
        attempt_count: usize,
        time_limit_info: Option<TimeLimitInfo>,
    ) {
        self.advance_y(5.0);
        let header_h = 12.0;

        match attempt_count {
            5 => self.draw_attempt_rows(&ATTEMPT_ROWS_5, header_h, time_limit_info),
            3 => self.draw_attempt_rows(&ATTEMPT_ROWS_3, header_h, time_limit_info),
            2 => self.draw_attempt_rows(&ATTEMPT_ROWS_2, header_h, time_limit_info),
            1 => self.draw_attempt_rows(&ATTEMPT_ROWS_1, header_h, time_limit_info),
            count => {
                let attempt_str_pool: Vec<String> = (1..=count).map(|i| i.to_string()).collect();
                let mut dynamic_row_storage: Vec<[&str; 5]> = Vec::with_capacity(count + 1);
                for s in &attempt_str_pool {
                    dynamic_row_storage.push([s.as_str(), "", "", "", ""]);
                }
                dynamic_row_storage.push(["Extra", "", "", "", ""]);

                let dynamic_rows: Vec<&[&str]> =
                    dynamic_row_storage.iter().map(|r| r.as_slice()).collect();
                self.draw_attempt_rows(&dynamic_rows, header_h, time_limit_info);
            }
        }
    }

    fn draw_attempt_rows(
        &mut self,
        rows: &[&[&str]],
        header_h: f32,
        time_limit_info: Option<TimeLimitInfo>,
    ) {
        let footer_reserve = if time_limit_info.is_some() { 10.0 } else { 0.0 };
        let available_h = self.cur_y - self.min_y - footer_reserve;
        let row_count = rows.len() as f32;
        let row_h = ((available_h - header_h) / row_count).clamp(13.5, 18.5);
        self.draw_grid_table(header_h, row_h, &ATTEMPT_COLUMNS, rows);

        if let Some(info) = time_limit_info {
            let formatted = info.format_display();
            self.draw_attempt_footer(&formatted);
        }
    }

    fn draw_attempt_footer(&mut self, info: &str) {
        let footer_y = (self.cur_y + self.min_y) / 2.0 - 2.0;
        TextDrawer::draw(
            self.ops,
            TextSpec {
                text: info,
                cell_x: self.inner_x,
                baseline_y: footer_y,
                cell_w: self.inner_w,
                font_size: 7.0,
                bold: false,
                align: TextAlign::Center,
            },
        );
    }

    /// Draws a styled horizontal section banner for cover sheets.
    pub fn draw_section_banner(&mut self, title: &str) {
        let banner_h = 13.0;
        let y_bot = self.cur_y - banner_h;

        // Background fill
        self.ops.push(Op::SetFillColor {
            col: Color::Greyscale(Greyscale::new(self.theme.header_bg_grey, None)),
        });
        self.ops.push(Op::DrawRectangle {
            rectangle: Rect {
                x: Pt(self.inner_x),
                y: Pt(y_bot),
                width: Pt(self.inner_w),
                height: Pt(banner_h),
                mode: Some(PaintMode::Fill),
                winding_order: None,
            },
        });

        // Top and bottom border lines
        self.ops.push(Op::SetOutlineColor {
            col: Color::Greyscale(Greyscale::new(self.theme.grid_line_grey, None)),
        });
        self.ops.push(Op::SetOutlineThickness {
            pt: Pt(self.theme.border_thickness),
        });

        // Top line
        self.ops.push(Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point {
                            x: Pt(self.inner_x),
                            y: Pt(self.cur_y),
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: Pt(self.inner_x + self.inner_w),
                            y: Pt(self.cur_y),
                        },
                        bezier: false,
                    },
                ],
                is_closed: false,
            },
        });

        // Bottom line
        self.ops.push(Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point {
                            x: Pt(self.inner_x),
                            y: Pt(y_bot),
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: Pt(self.inner_x + self.inner_w),
                            y: Pt(y_bot),
                        },
                        bezier: false,
                    },
                ],
                is_closed: false,
            },
        });

        // Banner text
        TextDrawer::draw(
            self.ops,
            TextSpec {
                text: title,
                cell_x: self.inner_x,
                baseline_y: y_bot + 3.5,
                cell_w: self.inner_w,
                font_size: 8.5,
                bold: true,
                align: TextAlign::Center,
            },
        );

        self.cur_y = y_bot;
    }

    /// Draws a square checkbox followed by a label on cover sheets.
    pub fn draw_checkbox_item(&mut self, text: &str) {
        let box_size = 8.0;
        let box_x = self.inner_x + 6.0;
        let box_y = self.cur_y - 1.0;

        self.ops.push(Op::SetOutlineColor {
            col: Color::Greyscale(Greyscale::new(0.2, None)),
        });
        self.ops.push(Op::SetOutlineThickness { pt: Pt(0.75) });
        self.ops.push(Op::DrawRectangle {
            rectangle: Rect {
                x: Pt(box_x),
                y: Pt(box_y),
                width: Pt(box_size),
                height: Pt(box_size),
                mode: Some(PaintMode::Stroke),
                winding_order: None,
            },
        });

        TextDrawer::draw(
            self.ops,
            TextSpec {
                text,
                cell_x: self.inner_x + 20.0,
                baseline_y: self.cur_y,
                cell_w: self.inner_w - 24.0,
                font_size: 8.5,
                bold: false,
                align: TextAlign::Left,
            },
        );
    }

    /// Draws a text label followed by a horizontal fill-in underline for signatures/initials.
    pub fn draw_field_with_line(&mut self, label: &str, indent: f32) {
        let text_x = self.inner_x + indent;
        let estimated_w = TextDrawer::estimate_width(label, 8.5);
        let line_start_x = text_x + estimated_w + 4.0;
        let line_end_x = self.inner_x + self.inner_w - 6.0;

        TextDrawer::draw(
            self.ops,
            TextSpec {
                text: label,
                cell_x: text_x,
                baseline_y: self.cur_y,
                cell_w: estimated_w + 4.0,
                font_size: 8.5,
                bold: false,
                align: TextAlign::Left,
            },
        );

        if line_end_x > line_start_x {
            self.ops.push(Op::SetOutlineColor {
                col: Color::Greyscale(Greyscale::new(0.4, None)),
            });
            self.ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });
            self.ops.push(Op::DrawLine {
                line: Line {
                    points: vec![
                        LinePoint {
                            p: Point {
                                x: Pt(line_start_x),
                                y: Pt(self.cur_y - 1.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(line_end_x),
                                y: Pt(self.cur_y - 1.0),
                            },
                            bezier: false,
                        },
                    ],
                    is_closed: false,
                },
            });
        }
    }

    /// Draws a complete cover sheet for a group with competition info, checkboxes, and signature fields.
    pub fn draw_cover_sheet(&mut self, card: &ScorecardItem<'_>) {
        self.draw_outer_border();

        // Top Header: Competition Name
        self.advance_y(6.0);
        TextDrawer::draw(
            self.ops,
            TextSpec {
                text: card.competition_name,
                cell_x: self.inner_x,
                baseline_y: self.cur_y,
                cell_w: self.inner_w,
                font_size: 11.5,
                bold: true,
                align: TextAlign::Center,
            },
        );

        // Top Header: Event & Round
        self.advance_y(14.0);
        let event_round_str = format!("{} Round {}", card.event_name, card.round_number);
        TextDrawer::draw(
            self.ops,
            TextSpec {
                text: &event_round_str,
                cell_x: self.inner_x,
                baseline_y: self.cur_y,
                cell_w: self.inner_w,
                font_size: 10.0,
                bold: true,
                align: TextAlign::Center,
            },
        );

        // Top Header: Group & Stage
        self.advance_y(13.0);
        let group_stage_str = if let Some(stage) = card.stage_name {
            format!("Group {} ({})", card.group_number, stage)
        } else {
            format!("Group {}", card.group_number)
        };
        TextDrawer::draw(
            self.ops,
            TextSpec {
                text: &group_stage_str,
                cell_x: self.inner_x,
                baseline_y: self.cur_y,
                cell_w: self.inner_w,
                font_size: 9.5,
                bold: true,
                align: TextAlign::Center,
            },
        );

        // --- FOR DELEGATE ---
        self.advance_y(14.0);
        self.draw_section_banner("FOR DELEGATE");

        self.advance_y(14.0);
        let bundle_str = format!("1. Bundled all {} scorecards", card.total_group_cards);
        self.draw_checkbox_item(&bundle_str);

        self.advance_y(13.0);
        self.draw_checkbox_item("2. Checked for missing signatures");

        self.advance_y(14.0);
        self.draw_field_with_line("3. Number of scorecards with incidents:", 6.0);

        self.advance_y(13.0);
        self.draw_field_with_line("Delegate initials:", 6.0);

        // --- FOR DATA ENTRY ---
        self.advance_y(16.0);
        self.draw_section_banner("FOR DATA ENTRY");

        self.advance_y(14.0);
        TextDrawer::draw(
            self.ops,
            TextSpec {
                text: "4. Results entered by Scoretaker",
                cell_x: self.inner_x + 6.0,
                baseline_y: self.cur_y,
                cell_w: self.inner_w - 12.0,
                font_size: 8.5,
                bold: false,
                align: TextAlign::Left,
            },
        );

        self.advance_y(12.0);
        self.draw_field_with_line("Scoretaker initials:", 18.0);

        self.advance_y(14.0);
        TextDrawer::draw(
            self.ops,
            TextSpec {
                text: "5. Incidents logged by Delegate",
                cell_x: self.inner_x + 6.0,
                baseline_y: self.cur_y,
                cell_w: self.inner_w - 12.0,
                font_size: 8.5,
                bold: false,
                align: TextAlign::Left,
            },
        );

        self.advance_y(12.0);
        self.draw_field_with_line("Delegate initials:", 18.0);

        self.advance_y(14.0);
        TextDrawer::draw(
            self.ops,
            TextSpec {
                text: "6. Results checked by Delegate",
                cell_x: self.inner_x + 6.0,
                baseline_y: self.cur_y,
                cell_w: self.inner_w - 12.0,
                font_size: 8.5,
                bold: false,
                align: TextAlign::Left,
            },
        );

        self.advance_y(12.0);
        self.draw_field_with_line("Delegate initials:", 18.0);
    }
}

/// Renderer for drawing scorecard elements and vector primitives to a PDF page instruction stream.
pub struct ScorecardRenderer;

impl ScorecardRenderer {
    /// Draws a complete scorecard or cover sheet within the given bounding rectangle (x, y, w, h).
    pub fn draw_card(ops: &mut Vec<Op>, card: &ScorecardItem<'_>, x: f32, y: f32, w: f32, h: f32) {
        let mut painter = CardPainter::new(ops, x, y, w, h, &DEFAULT_THEME);

        if card.is_cover_sheet {
            painter.draw_cover_sheet(card);
        } else {
            painter.draw_outer_border();
            painter.draw_top_header(card.scorecard_number, &card.truncated_competition_name(30));
            painter.draw_event_info_table(card);
            painter.draw_competitor_info_table(card);
            painter.draw_attempt_table(card.attempt_count, card.time_limit_info);
        }
    }
}

/// Helper struct for drawing grid tables with shaded headers and borders.
pub struct TableDrawer;

impl TableDrawer {
    /// Draws a styled grid table, advancing cur_y to the bottom of the table.
    pub fn draw(ops: &mut Vec<Op>, cur_y: &mut f32, spec: TableSpec<'_>, theme: &ScorecardTheme) {
        let top_y = *cur_y;
        let total_h = spec.header_h + spec.row_h * (spec.rows.len() as f32);
        let bottom_y = top_y - total_h;

        Self::draw_header_background(ops, &spec, top_y, theme);
        Self::draw_header_text(ops, &spec, top_y, theme);
        Self::draw_row_cells(ops, &spec, top_y, theme);
        Self::draw_grid_lines(ops, &spec, top_y, bottom_y, total_h, theme);

        *cur_y = bottom_y;
    }

    fn draw_header_background(
        ops: &mut Vec<Op>,
        spec: &TableSpec<'_>,
        top_y: f32,
        theme: &ScorecardTheme,
    ) {
        ops.push(Op::SetFillColor {
            col: Color::Greyscale(Greyscale::new(theme.header_bg_grey, None)),
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
    }

    fn draw_header_text(
        ops: &mut Vec<Op>,
        spec: &TableSpec<'_>,
        top_y: f32,
        theme: &ScorecardTheme,
    ) {
        let mut col_x = spec.tbl_x;
        let text_y = top_y - spec.header_h + (spec.header_h - theme.header_font_size) / 2.0 + 1.0;
        for (i, &header) in spec.headers.iter().enumerate() {
            let w = spec.col_widths[i];
            let align = spec.alignments.get(i).copied().unwrap_or(TextAlign::Center);
            TextDrawer::draw(
                ops,
                TextSpec {
                    text: header,
                    cell_x: col_x,
                    baseline_y: text_y,
                    cell_w: w,
                    font_size: theme.header_font_size,
                    bold: true,
                    align,
                },
            );
            col_x += w;
        }
    }

    fn draw_row_cells(ops: &mut Vec<Op>, spec: &TableSpec<'_>, top_y: f32, theme: &ScorecardTheme) {
        let mut row_top = top_y - spec.header_h;
        for &row in spec.rows {
            let mut cell_x = spec.tbl_x;
            let text_y = row_top - spec.row_h + (spec.row_h - theme.cell_font_size) / 2.0 + 1.0;
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
                        font_size: theme.cell_font_size,
                        bold: false,
                        align,
                    },
                );
                cell_x += w;
            }
            row_top -= spec.row_h;
        }
    }

    fn draw_grid_lines(
        ops: &mut Vec<Op>,
        spec: &TableSpec<'_>,
        top_y: f32,
        bottom_y: f32,
        total_h: f32,
        theme: &ScorecardTheme,
    ) {
        Self::set_grid_stroke_style(ops, theme);
        Self::draw_outer_table_border(ops, spec, bottom_y, total_h);
        Self::draw_horizontal_dividers(ops, spec, top_y);
        Self::draw_vertical_dividers(ops, spec, top_y, bottom_y);
    }

    fn set_grid_stroke_style(ops: &mut Vec<Op>, theme: &ScorecardTheme) {
        ops.push(Op::SetOutlineColor {
            col: Color::Greyscale(Greyscale::new(theme.grid_line_grey, None)),
        });
        ops.push(Op::SetOutlineThickness {
            pt: Pt(theme.grid_line_thickness),
        });
    }

    fn draw_outer_table_border(
        ops: &mut Vec<Op>,
        spec: &TableSpec<'_>,
        bottom_y: f32,
        total_h: f32,
    ) {
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
    }

    fn draw_horizontal_dividers(ops: &mut Vec<Op>, spec: &TableSpec<'_>, top_y: f32) {
        // Line after header
        Self::draw_line(
            ops,
            spec.tbl_x,
            top_y - spec.header_h,
            spec.tbl_x + spec.tbl_w,
            top_y - spec.header_h,
        );

        // Lines between rows
        let mut line_y = top_y - spec.header_h - spec.row_h;
        for _ in 1..spec.rows.len() {
            Self::draw_line(ops, spec.tbl_x, line_y, spec.tbl_x + spec.tbl_w, line_y);
            line_y -= spec.row_h;
        }
    }

    fn draw_vertical_dividers(ops: &mut Vec<Op>, spec: &TableSpec<'_>, top_y: f32, bottom_y: f32) {
        let mut sep_x = spec.tbl_x;
        for &w in &spec.col_widths[..spec.col_widths.len() - 1] {
            sep_x += w;
            Self::draw_line(ops, sep_x, top_y, sep_x, bottom_y);
        }
    }

    /// Draws a line between two points.
    /// NOTE: Each call allocates a small Vec for `LinePoint`s — this is a printpdf API
    /// requirement. For scorecard grids the count is bounded and the cost is negligible.
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
        let font = Self::resolve_font(spec.bold);
        let text_w = Self::estimate_width(spec.text, spec.font_size);
        let x = Self::compute_aligned_x(spec.align, spec.cell_x, spec.cell_w, text_w);
        Self::emit_text_ops(ops, font, spec.font_size, x, spec.baseline_y, spec.text);
    }

    fn resolve_font(bold: bool) -> PdfFontHandle {
        if bold {
            PdfFontHandle::Builtin(BuiltinFont::HelveticaBold)
        } else {
            PdfFontHandle::Builtin(BuiltinFont::Helvetica)
        }
    }

    fn compute_aligned_x(align: TextAlign, cell_x: f32, cell_w: f32, text_w: f32) -> f32 {
        let pad = 3.0;
        match align {
            TextAlign::Left => cell_x + pad,
            TextAlign::Center => cell_x + (cell_w - text_w).max(0.0) / 2.0,
        }
    }

    fn emit_text_ops(
        ops: &mut Vec<Op>,
        font: PdfFontHandle,
        font_size: f32,
        x: f32,
        y: f32,
        text: &str,
    ) {
        ops.push(Op::SetFillColor {
            col: Color::Greyscale(Greyscale::new(0.0, None)),
        });
        ops.push(Op::StartTextSection);
        ops.push(Op::SetFont {
            font,
            size: Pt(font_size),
        });
        ops.push(Op::SetTextCursor {
            pos: Point { x: Pt(x), y: Pt(y) },
        });
        ops.push(Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        });
        ops.push(Op::EndTextSection);
    }

    /// Approximates proportional Helvetica font character widths for centering.
    pub fn estimate_width(text: &str, font_size: f32) -> f32 {
        text.chars()
            .map(|ch| match ch {
                ' ' => 0.28,
                '.' | ',' | ':' | ';' | '!' | '|' | '\'' | '`' | 'i' | 'j' | 'l' | 'I' => 0.26,
                'f' | 't' | '(' | ')' | '[' | ']' | '{' | '}' => 0.32,
                'r' => 0.36,
                'm' | 'w' | 'M' | 'W' => 0.78,
                'A'..='Z' => 0.62,
                '0'..='9' => 0.55,
                _ => 0.50,
            })
            .sum::<f32>()
            * font_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scorecard::ScorecardItem;

    #[test]
    fn test_estimate_width() {
        let w_space = TextDrawer::estimate_width(" ", 10.0);
        assert!((w_space - 2.8).abs() < 0.01);

        let w_digits = TextDrawer::estimate_width("12345", 10.0);
        assert!((w_digits - 27.5).abs() < 0.01);

        let w_empty = TextDrawer::estimate_width("", 10.0);
        assert_eq!(w_empty, 0.0);
    }

    #[test]
    fn test_draw_card_operations() {
        let card = ScorecardItem {
            scorecard_number: 1,
            station_number: Some(4),
            competition_name: "Test Comp 2026",
            event_id: "333",
            event_name: "3x3x3 Cube",
            round_number: 1,
            group_number: 1,
            stage_name: Some("Red Stage"),
            competitor_name: "Alice Smith",
            registrant_id: Some(1),
            wca_id: Some("2022SMIT01"),
            attempt_count: 5,
            time_limit_info: Some(TimeLimitInfo {
                limit_centiseconds: Some(60000),
                is_cumulative: false,
                cutoff_centiseconds: None,
                cutoff_attempts: 0,
            }),
            is_blank: false,
            is_cover_sheet: false,
            total_group_cards: 0,
        };

        let mut ops = Vec::new();
        ScorecardRenderer::draw_card(&mut ops, &card, 18.0, 18.0, 270.0, 380.0);

        // Verify that operations were generated (borders, rects, text items)
        assert!(!ops.is_empty());
        let has_rectangles = ops.iter().any(|op| matches!(op, Op::DrawRectangle { .. }));
        let has_text = ops.iter().any(|op| matches!(op, Op::ShowText { .. }));
        assert!(has_rectangles);
        assert!(has_text);
    }

    #[test]
    fn test_draw_cover_sheet_operations() {
        let cover_card = ScorecardItem {
            scorecard_number: 0,
            station_number: None,
            competition_name: "Ocean State Cubikon 2025",
            event_id: "333",
            event_name: "3x3x3 Cube",
            round_number: 1,
            group_number: 1,
            stage_name: Some("Main Hall"),
            competitor_name: "",
            registrant_id: None,
            wca_id: None,
            attempt_count: 5,
            time_limit_info: None,
            is_blank: false,
            is_cover_sheet: true,
            total_group_cards: 15,
        };

        let mut ops = Vec::new();
        ScorecardRenderer::draw_card(&mut ops, &cover_card, 18.0, 18.0, 270.0, 380.0);

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
        let theme = DEFAULT_THEME;
        assert_eq!(theme.padding, 7.0);
        assert_eq!(theme.border_thickness, 0.75);
        assert!(theme.title_font_size > theme.header_font_size);
    }
}
