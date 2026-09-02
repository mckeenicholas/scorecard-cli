use crate::pdf::layout::RectSpec;
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
    header_font_size: 8.5,
    cell_font_size: 10.0,
    comp_name_font_size: 11.5,
};

/// Column specification for grid tables (header label, width ratio [0.0..1.0], alignment, bold cell flag).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnDef<'a> {
    pub header: &'a str,
    pub ratio: f32,
    pub align: TextAlign,
    pub bold: bool,
}

impl<'a> ColumnDef<'a> {
    #[inline]
    pub const fn new(header: &'a str, ratio: f32, align: TextAlign) -> Self {
        Self {
            header,
            ratio,
            align,
            bold: false,
        }
    }

    #[inline]
    pub const fn bold(header: &'a str, ratio: f32, align: TextAlign) -> Self {
        Self {
            header,
            ratio,
            align,
            bold: true,
        }
    }
}

/// Specification for rendering a structured grid table.
pub struct TableSpec<'a> {
    pub tbl_x: f32,
    pub tbl_w: f32,
    pub columns: &'a [ColumnDef<'a>],
    pub rows: &'a [&'a [&'a str]],
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

const ATTEMPT_COLUMNS: [ColumnDef<'static>; 5] = [
    ColumnDef::new("Attempt", 0.16, TextAlign::Center),
    ColumnDef::new("Scr", 0.15, TextAlign::Center),
    ColumnDef::new("Result", 0.39, TextAlign::Center),
    ColumnDef::new("Judge", 0.15, TextAlign::Center),
    ColumnDef::new("Comp", 0.15, TextAlign::Center),
];

/// Items rendered in the attempt table (regular solve rows, cutoff banners, extra banners).
#[derive(Debug, Clone, PartialEq)]
pub enum AttemptItem {
    Solve(String),
    CutoffBanner(String),
    ExtraBanner(String),
}

/// Specifications and precomputed geometry for the scorecard attempt table.
#[derive(Debug)]
pub struct AttemptTableSpec {
    pub items: Vec<AttemptItem>,
    pub row_h: f32,
    pub total_table_h: f32,
    pub col_widths: [f32; 5],
    pub footer_text: Option<String>,
}

#[inline]
fn grey(v: f32) -> Color {
    Color::Greyscale(Greyscale::new(v, None))
}

impl AttemptTableSpec {
    pub const HEADER_H: f32 = 12.0;
    pub const BANNER_H: f32 = 10.5;
    pub const BASE_ATTEMPT_ROWS: f32 = 6.0;

    pub fn build(
        inner_w: f32,
        available_h: f32,
        attempt_count: usize,
        time_limit_info: Option<TimeLimitInfo>,
    ) -> Self {
        let has_cutoff = time_limit_info.map_or(false, |info| {
            info.cutoff_centiseconds.is_some() && info.cutoff_attempts > 0
        });

        let cutoff_attempts = if has_cutoff {
            time_limit_info.map_or(0, |info| info.cutoff_attempts)
        } else {
            0
        };

        let footer_text = time_limit_info.and_then(|info| {
            info.limit_centiseconds.map(|cs| {
                let time_str = TimeLimitInfo::format_centiseconds(cs);
                if info.is_cumulative {
                    format!("Time limit: {} cumulative", time_str)
                } else {
                    format!("Time limit: {}", time_str)
                }
            })
        });

        let footer_reserve = if footer_text.is_some() { 12.0 } else { 0.0 };
        let usable_h = available_h - footer_reserve;
        let banner_count_5 = if has_cutoff { 2.0 } else { 1.0 };
        let row_h = ((usable_h - Self::HEADER_H - (banner_count_5 * Self::BANNER_H))
            / Self::BASE_ATTEMPT_ROWS)
            .max(13.5);

        let mut items = Vec::with_capacity(attempt_count + 3);
        for i in 1..=attempt_count {
            items.push(AttemptItem::Solve(i.to_string()));
            if has_cutoff && i == cutoff_attempts {
                if let Some(info) = time_limit_info {
                    if let Some(cs) = info.cutoff_centiseconds {
                        let cutoff_str = TimeLimitInfo::format_centiseconds(cs);
                        let format_name = if attempt_count <= 3 {
                            "mean"
                        } else {
                            "average"
                        };
                        items.push(AttemptItem::CutoffBanner(format!(
                            "-------- Must have solve under {} to complete {} --------",
                            cutoff_str, format_name
                        )));
                    }
                }
            }
        }

        items.push(AttemptItem::ExtraBanner(
            "Extra or provisional solve (Delegate initials: ______ )".to_string(),
        ));
        items.push(AttemptItem::Solve(String::new()));

        let total_banners = items
            .iter()
            .filter(|it| !matches!(it, AttemptItem::Solve(_)))
            .count() as f32;
        let total_attempts = (attempt_count + 1) as f32;
        let total_table_h =
            Self::HEADER_H + (total_attempts * row_h) + (total_banners * Self::BANNER_H);

        let mut col_widths = [0.0f32; 5];
        for (i, col) in ATTEMPT_COLUMNS.iter().enumerate() {
            col_widths[i] = inner_w * col.ratio;
        }

        Self {
            items,
            row_h,
            total_table_h,
            col_widths,
            footer_text,
        }
    }
}

/// Canvas abstraction managing vertical flow, bounding geometry, and rendering primitives for a scorecard.
pub struct CardPainter<'a> {
    pub ops: &'a mut Vec<Op>,
    pub theme: &'a ScorecardTheme,
    pub bounds: RectSpec,
    pub inner_x: f32,
    pub inner_w: f32,
    pub cur_y: f32,
    pub min_y: f32,
}

impl<'a> CardPainter<'a> {
    pub fn new(
        ops: &'a mut Vec<Op>,
        bounds: RectSpec,
        theme: &'a ScorecardTheme,
    ) -> Self {
        let pad = theme.padding;
        let inner_x = bounds.x + pad;
        let inner_w = bounds.w - 2.0 * pad;
        let cur_y = bounds.y + bounds.h - pad;
        let min_y = bounds.y + pad;

        Self {
            ops,
            theme,
            bounds,
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

    #[inline]
    pub fn set_outline(&mut self, val: f32, thickness: f32) {
        self.ops.push(Op::SetOutlineColor {
            col: grey(val),
        });
        self.ops.push(Op::SetOutlineThickness { pt: Pt(thickness) });
    }

    #[inline]
    pub fn set_fill(&mut self, val: f32) {
        self.ops.push(Op::SetFillColor {
            col: grey(val),
        });
    }

    #[inline]
    pub fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        TableDrawer::draw_line(self.ops, x1, y1, x2, y2);
    }

    #[inline]
    pub fn draw_filled_rect(&mut self, x: f32, y: f32, w: f32, h: f32, val: f32) {
        self.set_fill(val);
        self.ops.push(Op::DrawRectangle {
            rectangle: Rect {
                x: Pt(x),
                y: Pt(y),
                width: Pt(w),
                height: Pt(h),
                mode: Some(PaintMode::Fill),
                winding_order: None,
            },
        });
    }

    #[inline]
    pub fn draw_stroked_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.ops.push(Op::DrawRectangle {
            rectangle: Rect {
                x: Pt(x),
                y: Pt(y),
                width: Pt(w),
                height: Pt(h),
                mode: Some(PaintMode::Stroke),
                winding_order: None,
            },
        });
    }

    /// Draws centered, full-width text across the card's printable horizontal area.
    #[inline]
    pub fn draw_full_width(&mut self, text: &str, baseline_y: f32, font_size: f32, bold: bool) {
        TextDrawer::draw(
            self.ops,
            TextSpec {
                text,
                cell_x: self.inner_x,
                baseline_y,
                cell_w: self.inner_w,
                font_size,
                bold,
                align: TextAlign::Center,
            },
        );
    }

    /// Draws the outer scorecard bounding box border.
    pub fn draw_outer_border(&mut self) {
        self.set_outline(0.0, self.theme.border_thickness);
        self.draw_stroked_rect(self.bounds.x, self.bounds.y, self.bounds.w, self.bounds.h);
    }

    /// Draws the top header: scorecard number in top-left corner and competition name centered.
    pub fn draw_top_header(&mut self, scorecard_number: usize, comp_name: &str) {
        self.advance_y(12.0);

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
                    bold: false,
                    align: TextAlign::Left,
                },
            );
        }

        self.draw_full_width(
            comp_name,
            self.cur_y,
            self.theme.comp_name_font_size,
            true,
        );

        self.advance_y(8.0);
    }

    /// Draws a structured grid table with specified column definitions, header height, and row height.
    pub fn draw_grid_table(
        &mut self,
        header_h: f32,
        row_h: f32,
        columns: &[ColumnDef<'_>],
        rows: &[&[&str]],
    ) {
        TableDrawer::draw(
            self.ops,
            &mut self.cur_y,
            TableSpec {
                tbl_x: self.inner_x,
                tbl_w: self.inner_w,
                columns,
                rows,
                header_h,
                row_h,
            },
            self.theme,
        );
    }

    /// Draws the event, round, group, and station info grid table.
    /// If there is no station number (or station numbers are disabled), renders a 3-column table.
    pub fn draw_event_info_table(&mut self, card: &ScorecardItem<'_>) {
        let mut round_buf = itoa::Buffer::new();
        let round_str = round_buf.format(card.round_number);

        let mut group_buf = itoa::Buffer::new();
        let group_str = group_buf.format(card.group_number);

        if let Some(station) = card.station_number {
            let mut station_buf = itoa::Buffer::new();
            let station_val = station_buf.format(station);
            self.draw_grid_table(
                14.5,
                18.0,
                &[
                    ColumnDef::new("Event", 0.38, TextAlign::Left),
                    ColumnDef::new("Round", 0.20, TextAlign::Center),
                    ColumnDef::new("Group", 0.20, TextAlign::Center),
                    ColumnDef::new("Station", 0.22, TextAlign::Center),
                ],
                &[&[card.event_name, round_str, group_str, station_val]],
            );
        } else {
            self.draw_grid_table(
                14.5,
                18.0,
                &[
                    ColumnDef::new("Event", 0.50, TextAlign::Left),
                    ColumnDef::new("Round", 0.25, TextAlign::Center),
                    ColumnDef::new("Group", 0.25, TextAlign::Center),
                ],
                &[&[card.event_name, round_str, group_str]],
            );
        }
    }

    /// Draws the competitor ID, name, and WCA ID grid table.
    /// The competitor name is rendered in bold.
    pub fn draw_competitor_info_table(&mut self, card: &ScorecardItem<'_>) {
        self.advance_y(5.0);
        let mut id_buf = itoa::Buffer::new();
        let id_val = card
            .registrant_id
            .map(|id| id_buf.format(id))
            .unwrap_or("-");
        let name_val = card.display_competitor_name();
        let wca_id_val = card.display_wca_id();

        self.draw_grid_table(
            14.5,
            20.0,
            &[
                ColumnDef::new("ID", 0.16, TextAlign::Center),
                ColumnDef::bold("Competitor Name", 0.54, TextAlign::Left),
                ColumnDef::new("WCA ID", 0.30, TextAlign::Center),
            ],
            &[&[id_val, name_val, wca_id_val]],
        );
    }

    /// Draws the attempt table dynamically sized to fill the remaining scorecard height,
    /// with inline cutoff banner, extra solve banner, blank extra box, and bottom time limit footer.
    pub fn draw_attempt_table(
        &mut self,
        attempt_count: usize,
        time_limit_info: Option<TimeLimitInfo>,
    ) {
        self.advance_y(5.0);
        let spec = AttemptTableSpec::build(
            self.inner_w,
            self.cur_y - self.min_y,
            attempt_count,
            time_limit_info,
        );

        let top_y = self.cur_y;
        let bottom_y = top_y - spec.total_table_h;

        self.draw_attempt_header(top_y, &spec);
        self.draw_attempt_items(top_y, &spec);
        self.draw_attempt_border(bottom_y, spec.total_table_h);

        self.cur_y = bottom_y;

        if let Some(footer_msg) = &spec.footer_text {
            self.draw_attempt_footer(footer_msg);
        }
    }

    fn draw_attempt_header(&mut self, top_y: f32, spec: &AttemptTableSpec) {
        let header_h = AttemptTableSpec::HEADER_H;
        self.draw_filled_rect(
            self.inner_x,
            top_y - header_h,
            self.inner_w,
            header_h,
            self.theme.header_bg_grey,
        );

        let mut h_col_x = self.inner_x;
        let h_text_y = top_y - header_h + (header_h - self.theme.header_font_size) / 2.0 + 1.0;
        for (i, col) in ATTEMPT_COLUMNS.iter().enumerate() {
            let w = spec.col_widths[i];
            TextDrawer::draw(
                self.ops,
                TextSpec {
                    text: col.header,
                    cell_x: h_col_x,
                    baseline_y: h_text_y,
                    cell_w: w,
                    font_size: self.theme.header_font_size,
                    bold: true,
                    align: col.align,
                },
            );
            h_col_x += w;
        }

        self.set_grid_stroke();
        self.draw_line(
            self.inner_x,
            top_y - header_h,
            self.inner_x + self.inner_w,
            top_y - header_h,
        );
    }

    #[inline]
    fn set_grid_stroke(&mut self) {
        self.set_outline(self.theme.grid_line_grey, self.theme.grid_line_thickness);
    }

    fn draw_attempt_items(&mut self, top_y: f32, spec: &AttemptTableSpec) {
        let mut cur_row_y = top_y - AttemptTableSpec::HEADER_H;

        for item in &spec.items {
            match item {
                AttemptItem::Solve(attempt_label) => {
                    let next_y = cur_row_y - spec.row_h;

                    if !attempt_label.is_empty() {
                        let text_y = next_y + (spec.row_h - self.theme.cell_font_size) / 2.0 + 1.0;
                        TextDrawer::draw(
                            self.ops,
                            TextSpec {
                                text: attempt_label,
                                cell_x: self.inner_x,
                                baseline_y: text_y,
                                cell_w: spec.col_widths[0],
                                font_size: self.theme.cell_font_size,
                                bold: false,
                                align: TextAlign::Center,
                            },
                        );
                    }

                    self.set_grid_stroke();
                    self.draw_line(self.inner_x, next_y, self.inner_x + self.inner_w, next_y);

                    let mut div_x = self.inner_x;
                    for &w in spec.col_widths.iter().take(spec.col_widths.len() - 1) {
                        div_x += w;
                        self.draw_line(div_x, cur_row_y, div_x, next_y);
                    }

                    cur_row_y = next_y;
                }
                AttemptItem::CutoffBanner(text) | AttemptItem::ExtraBanner(text) => {
                    let next_y = cur_row_y - AttemptTableSpec::BANNER_H;

                    self.draw_filled_rect(
                        self.inner_x,
                        next_y,
                        self.inner_w,
                        AttemptTableSpec::BANNER_H,
                        self.theme.header_bg_grey,
                    );

                    let text_y = next_y + (AttemptTableSpec::BANNER_H - 7.0) / 2.0 + 1.0;
                    self.draw_full_width(text, text_y, 7.0, false);

                    self.set_grid_stroke();
                    self.draw_line(self.inner_x, next_y, self.inner_x + self.inner_w, next_y);

                    cur_row_y = next_y;
                }
            }
        }
    }

    fn draw_attempt_border(&mut self, bottom_y: f32, total_table_h: f32) {
        self.set_grid_stroke();
        self.draw_stroked_rect(self.inner_x, bottom_y, self.inner_w, total_table_h);
    }

    fn draw_attempt_footer(&mut self, info: &str) {
        let footer_y = self.min_y + 1.0;
        self.draw_full_width(info, footer_y, 7.0, false);
    }

    /// Draws a styled horizontal section banner for cover sheets.
    pub fn draw_section_banner(&mut self, title: &str) {
        let banner_h = 13.0;
        let y_bot = self.cur_y - banner_h;

        self.draw_filled_rect(
            self.inner_x,
            y_bot,
            self.inner_w,
            banner_h,
            self.theme.header_bg_grey,
        );

        self.set_outline(self.theme.grid_line_grey, self.theme.border_thickness);
        self.draw_line(self.inner_x, self.cur_y, self.inner_x + self.inner_w, self.cur_y);
        self.draw_line(self.inner_x, y_bot, self.inner_x + self.inner_w, y_bot);

        self.draw_full_width(title, y_bot + 3.5, 8.5, true);

        self.cur_y = y_bot;
    }

    /// Draws a square checkbox followed by a label on cover sheets (centered).
    pub fn draw_checkbox_item(&mut self, text: &str) {
        let box_size = 8.0f32;
        let gap = 6.0f32;
        let text_w = TextDrawer::estimate_width(text, 8.5);
        let total_w = box_size + gap + text_w;
        let start_x = self.inner_x + (self.inner_w - total_w) / 2.0;
        let box_y = self.cur_y - 1.0;

        self.set_outline(0.2, 0.75);
        self.draw_stroked_rect(start_x, box_y, box_size, box_size);

        TextDrawer::draw(
            self.ops,
            TextSpec {
                text,
                cell_x: start_x + box_size + gap,
                baseline_y: self.cur_y,
                cell_w: text_w + 4.0,
                font_size: 8.5,
                bold: false,
                align: TextAlign::Left,
            },
        );
    }

    /// Draws a text label followed by a horizontal fill-in underline for signatures/initials (centered).
    /// The underline area is ~4 characters wide (~30pt).
    pub fn draw_field_with_line(&mut self, label: &str, _indent: f32) {
        let estimated_w = TextDrawer::estimate_width(label, 8.5);
        let line_w = 30.0f32; // ~4 characters wide
        let gap = 5.0f32;
        let total_w = estimated_w + gap + line_w;
        let start_x = self.inner_x + (self.inner_w - total_w) / 2.0;

        TextDrawer::draw(
            self.ops,
            TextSpec {
                text: label,
                cell_x: start_x,
                baseline_y: self.cur_y,
                cell_w: estimated_w + 2.0,
                font_size: 8.5,
                bold: false,
                align: TextAlign::Left,
            },
        );

        let line_start_x = start_x + estimated_w + gap;
        let line_end_x = start_x + total_w;

        self.set_outline(0.4, 0.5);
        self.draw_line(
            line_start_x,
            self.cur_y - 1.0,
            line_end_x,
            self.cur_y - 1.0,
        );
    }

    /// Draws a complete competitor scorecard.
    pub fn draw_competitor_card(&mut self, card: &ScorecardItem<'_>) {
        self.draw_outer_border();
        self.draw_top_header(card.scorecard_number, &card.truncated_competition_name(30));
        self.draw_event_info_table(card);
        self.draw_competitor_info_table(card);
        self.draw_attempt_table(card.attempt_count, card.time_limit_info);
    }

    /// Draws a complete cover sheet for a group with competition info, checkboxes, and signature fields.
    pub fn draw_cover_sheet(&mut self, card: &ScorecardItem<'_>) {
        self.draw_outer_border();
        self.draw_cover_sheet_header(card);
        self.draw_delegate_section(card.total_group_cards);
        self.draw_data_entry_section();
    }

    fn draw_cover_sheet_header(&mut self, card: &ScorecardItem<'_>) {
        self.advance_y(6.0);
        self.draw_full_width(card.competition_name, self.cur_y, 11.5, true);

        self.advance_y(14.0);
        let event_round_str = format!("{} Round {}", card.event_name, card.round_number);
        self.draw_full_width(&event_round_str, self.cur_y, 10.0, true);

        let group_stage_str = match (card.group_number, card.stage_name) {
            (g, Some(stage)) if g > 0 => format!("Group {} ({})", g, stage),
            (g, None) if g > 0 => format!("Group {}", g),
            (0, Some(stage)) => format!("Stage: {}", stage),
            (0, None) => String::new(),
            _ => String::new(),
        };
        if !group_stage_str.is_empty() {
            self.advance_y(13.0);
            self.draw_full_width(&group_stage_str, self.cur_y, 9.5, true);
        }
    }

    fn draw_delegate_section(&mut self, total_cards: usize) {
        self.advance_y(14.0);
        self.draw_section_banner("FOR DELEGATE");

        self.advance_y(14.0);
        let bundle_str = format!("1. Bundled all {} scorecards", total_cards);
        self.draw_checkbox_item(&bundle_str);

        self.advance_y(13.0);
        self.draw_checkbox_item("2. Checked for missing signatures");

        self.advance_y(14.0);
        self.draw_field_with_line("3. Number of scorecards with incidents:", 6.0);

        self.advance_y(13.0);
        self.draw_field_with_line("Delegate initials:", 6.0);
    }

    fn draw_data_entry_section(&mut self) {
        self.advance_y(16.0);
        self.draw_section_banner("FOR DATA ENTRY");

        self.advance_y(14.0);
        self.draw_step_header("4. Results entered by Scoretaker");
        self.advance_y(12.0);
        self.draw_field_with_line("Scoretaker initials:", 18.0);

        self.advance_y(14.0);
        self.draw_step_header("5. Incidents logged by Delegate");
        self.advance_y(12.0);
        self.draw_field_with_line("Delegate initials:", 18.0);

        self.advance_y(14.0);
        self.draw_step_header("6. Results checked by Delegate");
        self.advance_y(12.0);
        self.draw_field_with_line("Delegate initials:", 18.0);
    }

    fn draw_step_header(&mut self, text: &str) {
        self.draw_full_width(text, self.cur_y, 8.5, false);
    }
}

/// Renderer for drawing scorecard elements and vector primitives to a PDF page instruction stream.
pub struct ScorecardRenderer;

impl ScorecardRenderer {
    /// Draws a complete scorecard or cover sheet within the given bounding rectangle.
    #[inline]
    pub fn draw_card_rect(ops: &mut Vec<Op>, card: &ScorecardItem<'_>, bounds: RectSpec) {
        let mut painter = CardPainter::new(ops, bounds, &DEFAULT_THEME);
        if card.is_cover_sheet {
            painter.draw_cover_sheet(card);
        } else {
            painter.draw_competitor_card(card);
        }
    }

    /// Draws a complete scorecard or cover sheet within the given bounding rectangle (x, y, w, h).
    #[inline]
    #[allow(dead_code)]
    pub fn draw_card(ops: &mut Vec<Op>, card: &ScorecardItem<'_>, x: f32, y: f32, w: f32, h: f32) {
        Self::draw_card_rect(ops, card, RectSpec { x, y, w, h });
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
            col: grey(theme.header_bg_grey),
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
        for col in spec.columns {
            let w = col.ratio * spec.tbl_w;
            TextDrawer::draw(
                ops,
                TextSpec {
                    text: col.header,
                    cell_x: col_x,
                    baseline_y: text_y,
                    cell_w: w,
                    font_size: theme.header_font_size,
                    bold: true,
                    align: col.align,
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
                let col = spec.columns.get(i);
                let w = col.map_or(0.0, |c| c.ratio * spec.tbl_w);
                let align = col.map_or(TextAlign::Center, |c| c.align);
                let bold = col.map_or(false, |c| c.bold);
                TextDrawer::draw(
                    ops,
                    TextSpec {
                        text: cell,
                        cell_x,
                        baseline_y: text_y,
                        cell_w: w,
                        font_size: theme.cell_font_size,
                        bold,
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
            col: grey(theme.grid_line_grey),
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
        for col in spec.columns.iter().take(spec.columns.len().saturating_sub(1)) {
            sep_x += col.ratio * spec.tbl_w;
            Self::draw_line(ops, sep_x, top_y, sep_x, bottom_y);
        }
    }

    /// Draws a line between two points.
    /// NOTE: Each call allocates a small Vec for `LinePoint`s — this is a printpdf API
    /// requirement. For scorecard grids the count is bounded and the cost is negligible.
    pub fn draw_line(ops: &mut Vec<Op>, x1: f32, y1: f32, x2: f32, y2: f32) {
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
            col: grey(0.0),
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
        text.chars().map(Self::char_raw_width).sum::<f32>() * font_size
    }

    fn char_raw_width(ch: char) -> f32 {
        match ch {
            ' ' => 0.28,
            '.' | ',' | ':' | ';' | '!' | '|' | '\'' | '`' | 'i' | 'j' | 'l' | 'I' => 0.26,
            'f' | 't' | '(' | ')' | '[' | ']' | '{' | '}' => 0.32,
            'r' => 0.36,
            'm' | 'w' | 'M' | 'W' => 0.78,
            'A'..='Z' => 0.62,
            '0'..='9' => 0.55,
            _ => 0.50,
        }
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

        // Verify competitor name is drawn in bold font
        let has_bold_name = ops.windows(3).any(|window| {
            let is_bold_font = matches!(&window[0], Op::SetFont { font: printpdf::PdfFontHandle::Builtin(printpdf::BuiltinFont::HelveticaBold), .. });
            let has_alice = matches!(&window[2], Op::ShowText { items } if items.iter().any(|it| match it {
                printpdf::ops::TextItem::Text(s) => s == "Alice Smith",
                _ => false,
            }));
            is_bold_font && has_alice
        });
        assert!(has_bold_name, "Competitor name should be rendered in bold font");
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

    #[test]
    fn test_event_info_table_3_cols_without_station() {
        let card_no_station = ScorecardItem {
            scorecard_number: 1,
            station_number: None,
            competition_name: "Test Comp 2026",
            event_id: "333",
            event_name: "3x3x3 Cube",
            round_number: 1,
            group_number: 1,
            stage_name: None,
            competitor_name: "Alice Smith",
            registrant_id: Some(1),
            wca_id: None,
            attempt_count: 5,
            time_limit_info: None,
            is_blank: false,
            is_cover_sheet: false,
            total_group_cards: 0,
        };

        let mut ops_no_station = Vec::new();
        ScorecardRenderer::draw_card(
            &mut ops_no_station,
            &card_no_station,
            18.0,
            18.0,
            270.0,
            380.0,
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

        let card_with_station = ScorecardItem {
            station_number: Some(3),
            ..card_no_station
        };
        let mut ops_with_station = Vec::new();
        ScorecardRenderer::draw_card(
            &mut ops_with_station,
            &card_with_station,
            18.0,
            18.0,
            270.0,
            380.0,
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
        let card = ScorecardItem {
            scorecard_number: 1,
            station_number: None,
            competition_name: "Test Comp 2026",
            event_id: "333",
            event_name: "3x3x3 Cube",
            round_number: 1,
            group_number: 1,
            stage_name: None,
            competitor_name: "Alice Smith",
            registrant_id: Some(1),
            wca_id: None,
            attempt_count: 5,
            time_limit_info: Some(TimeLimitInfo {
                limit_centiseconds: Some(60000),
                is_cumulative: false,
                cutoff_centiseconds: Some(4500),
                cutoff_attempts: 2,
            }),
            is_blank: false,
            is_cover_sheet: false,
            total_group_cards: 0,
        };

        let mut ops = Vec::new();
        ScorecardRenderer::draw_card(&mut ops, &card, 18.0, 18.0, 270.0, 380.0);

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
}
