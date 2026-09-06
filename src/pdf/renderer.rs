use crate::pdf::layout::RectSpec;
use crate::scorecard::{
    BlankScorecard, Competitor, CoverSheet, Scorecard, ScorecardItem, TimeLimitInfo,
};
use printpdf::FontId;
use printpdf::color::{Color, Greyscale};
use printpdf::font::BuiltinFont;
use printpdf::graphics::{Line, LinePoint, PaintMode, Point, Rect};
use printpdf::ops::{Op, PdfFontHandle};
use printpdf::text::TextItem;
use printpdf::units::Pt;
use std::fmt::Write;

/// Text alignment within a scorecard cell or bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
}

/// Visual theme and geometric styling parameters for scorecard rendering.
#[derive(Debug, Clone, PartialEq)]
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
    pub custom_font: Option<FontId>,
}

impl ScorecardTheme {
    pub fn with_font(mut self, font: Option<FontId>) -> Self {
        self.custom_font = font;
        self
    }
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
    custom_font: None,
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableSpec<'a> {
    pub tbl_x: f32,
    pub tbl_w: f32,
    pub columns: &'a [ColumnDef<'a>],
    pub rows: &'a [&'a [&'a str]],
    pub header_h: f32,
    pub row_h: f32,
}

/// Specification for rendering text with alignment and font styling.
#[derive(Debug, Clone, Copy, PartialEq)]
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

/// Standard labels for regular attempt solve rows (avoids per-card integer-to-string allocations).
pub const ATTEMPT_LABELS: [&str; 5] = ["1", "2", "3", "4", "5"];

/// Specifications and precomputed geometry for the scorecard attempt table.
#[derive(Debug)]
pub struct AttemptTableSpec {
    pub attempt_count: usize,
    pub cutoff_attempts: usize,
    pub cutoff_banner: Option<String>,
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
        let has_cutoff = time_limit_info
            .is_some_and(|info| info.cutoff_centiseconds.is_some() && info.cutoff_attempts > 0);

        let cutoff_attempts = if has_cutoff {
            time_limit_info.map_or(0, |info| info.cutoff_attempts)
        } else {
            0
        };

        let footer_text = time_limit_info.and_then(|info| {
            info.limit_centiseconds.map(|limit| {
                let mut text = String::with_capacity(32);
                if info.is_cumulative {
                    let _ = write!(text, "Time limit: {limit} cumulative");
                } else {
                    let _ = write!(text, "Time limit: {limit}");
                }
                text
            })
        });

        let footer_reserve = if footer_text.is_some() { 12.0 } else { 0.0 };
        let usable_h = available_h - footer_reserve;
        let banner_count_5 = if has_cutoff { 2.0 } else { 1.0 };
        let row_h = ((usable_h - Self::HEADER_H - (banner_count_5 * Self::BANNER_H))
            / Self::BASE_ATTEMPT_ROWS)
            .max(13.5);

        let cutoff_banner = if has_cutoff {
            let cutoff = time_limit_info
                .and_then(|info| info.cutoff_centiseconds)
                .unwrap_or_default();
            let format_name = if attempt_count <= 3 {
                "mean"
            } else {
                "average"
            };
            let mut banner = String::with_capacity(80);
            let _ = write!(
                banner,
                "-------- Must have solve under {cutoff} to complete {format_name} --------"
            );
            Some(banner)
        } else {
            None
        };

        let total_banners = if has_cutoff { 2.0 } else { 1.0 };
        let total_attempts = f32::from(u16::try_from(attempt_count + 1).unwrap_or(6));
        let total_table_h =
            Self::HEADER_H + (total_attempts * row_h) + (total_banners * Self::BANNER_H);

        let col_widths = ATTEMPT_COLUMNS.map(|col| inner_w * col.ratio);

        Self {
            attempt_count,
            cutoff_attempts,
            cutoff_banner,
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
    pub fn new(ops: &'a mut Vec<Op>, bounds: RectSpec, theme: &'a ScorecardTheme) -> Self {
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
        self.ops.push(Op::SetOutlineColor { col: grey(val) });
        self.ops.push(Op::SetOutlineThickness { pt: Pt(thickness) });
    }

    #[inline]
    pub fn set_fill(&mut self, val: f32) {
        self.ops.push(Op::SetFillColor { col: grey(val) });
    }

    #[inline]
    pub fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        TableDrawer::draw_line(self.ops, x1, y1, x2, y2);
    }

    #[inline]
    pub fn draw_filled_rect(&mut self, rect: RectSpec, val: f32) {
        self.set_fill(val);
        TableDrawer::draw_rect(self.ops, rect, PaintMode::Fill);
    }

    #[inline]
    pub fn draw_stroked_rect(&mut self, rect: RectSpec) {
        TableDrawer::draw_rect(self.ops, rect, PaintMode::Stroke);
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
        self.draw_stroked_rect(self.bounds);
    }

    /// Draws the top header: scorecard number in top-left corner and competition name centered.
    pub fn draw_top_header(&mut self, number: usize, comp_name: &str) {
        self.advance_y(12.0);

        if number > 0 {
            let mut num_buf = itoa::Buffer::new();
            let num_str = num_buf.format(number);
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

        self.draw_full_width(comp_name, self.cur_y, self.theme.comp_name_font_size, true);

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
    pub fn draw_event_info_table(
        &mut self,
        event_name: &str,
        round_number: usize,
        group_number: usize,
        station_number: Option<usize>,
    ) {
        let mut round_buf = itoa::Buffer::new();
        let round_str = round_buf.format(round_number);

        let mut group_buf = itoa::Buffer::new();
        let group_str = group_buf.format(group_number);

        if let Some(station) = station_number {
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
                &[&[event_name, round_str, group_str, station_val]],
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
                &[&[event_name, round_str, group_str]],
            );
        }
    }

    /// Draws the competitor ID, name, and WCA ID grid table for an assigned competitor.
    /// The competitor name is rendered in bold.
    pub fn draw_competitor_info_table(&mut self, competitor: &Competitor<'_>) {
        self.advance_y(5.0);
        let mut id_buf = itoa::Buffer::new();
        let id_val = id_buf.format(competitor.registrant_id.get());
        let wca_id_val = competitor.display_wca_id();

        let col_defs = &[
            ColumnDef::new("ID", 0.16, TextAlign::Center),
            ColumnDef::bold("Competitor Name", 0.54, TextAlign::Left),
            ColumnDef::new("WCA ID", 0.30, TextAlign::Center),
        ][..];

        let rows: &[&[&str]] = &[&[id_val, "", wca_id_val]];
        let row_top = self.cur_y - 14.5;
        self.draw_grid_table(14.5, 20.0, col_defs, rows);

        if !competitor.name.is_empty() || competitor.local_name.is_some() {
            let cell_x = self.inner_x + 0.16 * self.inner_w;
            let cell_w = 0.54 * self.inner_w;
            self.draw_competitor_name(
                competitor.name,
                competitor.local_name,
                cell_x,
                row_top,
                cell_w,
                20.0,
            );
        }
    }

    /// Draws the blank competitor ID and name table for blank scorecards.
    /// WCA ID is omitted and ID is empty to maximize space for writing the competitor's name.
    pub fn draw_blank_competitor_info_table(&mut self) {
        self.advance_y(5.0);
        let col_defs = &[
            ColumnDef::new("ID", 0.16, TextAlign::Center),
            ColumnDef::bold("Competitor Name", 0.84, TextAlign::Left),
        ][..];

        let rows: &[&[&str]] = &[&["", ""]];
        self.draw_grid_table(14.5, 20.0, col_defs, rows);
    }

    fn draw_competitor_name(
        &mut self,
        primary: &str,
        local: Option<&str>,
        cell_x: f32,
        row_top: f32,
        cell_w: f32,
        row_h: f32,
    ) {
        let max_w = (cell_w - 6.0).max(10.0);
        let full_w =
            TextDrawer::estimate_competitor_name_width(primary, local, self.theme.cell_font_size);
        let font_size = if full_w > max_w {
            (self.theme.cell_font_size * (max_w / full_w)).max(6.0)
        } else {
            self.theme.cell_font_size
        };

        let baseline_y = row_top - row_h + (row_h - font_size) / 2.0 + 1.0;
        let pad = 3.0;
        let start_x = cell_x + pad;

        TextDrawer::draw_competitor_name(
            self.ops,
            primary,
            local,
            start_x,
            baseline_y,
            font_size,
            self.theme.custom_font.as_ref(),
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
            RectSpec::new(self.inner_x, top_y - header_h, self.inner_w, header_h),
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
                    bold: false,
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

        for i in 1..=spec.attempt_count {
            let label = if (1..=5).contains(&i) {
                ATTEMPT_LABELS[i - 1]
            } else {
                ""
            };
            cur_row_y = self.draw_solve_row(cur_row_y, label, spec);

            if i == spec.cutoff_attempts
                && let Some(ref banner) = spec.cutoff_banner
            {
                cur_row_y = self.draw_banner_row(cur_row_y, banner);
            }
        }

        cur_row_y = self.draw_banner_row(
            cur_row_y,
            "Extra or provisional solve (Delegate initials: ______ )",
        );
        self.draw_solve_row(cur_row_y, "", spec);
    }

    fn draw_solve_row(
        &mut self,
        cur_row_y: f32,
        attempt_label: &str,
        spec: &AttemptTableSpec,
    ) -> f32 {
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

        next_y
    }

    fn draw_banner_row(&mut self, cur_row_y: f32, text: &str) -> f32 {
        let next_y = cur_row_y - AttemptTableSpec::BANNER_H;

        self.draw_filled_rect(
            RectSpec::new(
                self.inner_x,
                next_y,
                self.inner_w,
                AttemptTableSpec::BANNER_H,
            ),
            self.theme.header_bg_grey,
        );

        let text_y = next_y + (AttemptTableSpec::BANNER_H - 7.0) / 2.0 + 1.0;
        self.draw_full_width(text, text_y, 7.0, false);

        self.set_grid_stroke();
        self.draw_line(self.inner_x, next_y, self.inner_x + self.inner_w, next_y);

        next_y
    }

    fn draw_attempt_border(&mut self, bottom_y: f32, total_table_h: f32) {
        self.set_grid_stroke();
        self.draw_stroked_rect(RectSpec::new(
            self.inner_x,
            bottom_y,
            self.inner_w,
            total_table_h,
        ));
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
            RectSpec::new(self.inner_x, y_bot, self.inner_w, banner_h),
            self.theme.header_bg_grey,
        );

        self.set_outline(self.theme.grid_line_grey, self.theme.border_thickness);
        self.draw_line(
            self.inner_x,
            self.cur_y,
            self.inner_x + self.inner_w,
            self.cur_y,
        );
        self.draw_line(self.inner_x, y_bot, self.inner_x + self.inner_w, y_bot);

        self.draw_full_width(title, y_bot + 3.5, 8.5, true);

        self.cur_y = y_bot;
    }

    /// Draws a square checkbox followed by a label on cover sheets (centered).
    pub fn draw_checkbox_item(&mut self, text: &str) {
        let box_size = 8.0f32;
        let gap = 6.0f32;
        let text_w = TextDrawer::estimate_width(text, 8.5, false);
        let total_w = box_size + gap + text_w;
        let start_x = self.inner_x + (self.inner_w - total_w) / 2.0;
        let box_y = self.cur_y - 1.0;

        self.set_outline(0.2, 0.75);
        self.draw_stroked_rect(RectSpec::new(start_x, box_y, box_size, box_size));

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
        let estimated_w = TextDrawer::estimate_width(label, 8.5, false);
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
        self.draw_line(line_start_x, self.cur_y - 1.0, line_end_x, self.cur_y - 1.0);
    }

    /// Draws a complete competitor scorecard.
    pub fn draw_scorecard(&mut self, card: &Scorecard<'_>) {
        self.draw_outer_border();
        self.draw_top_header(card.number, &card.truncated_competition_name(30));
        self.draw_event_info_table(
            card.event_name(),
            card.round_number,
            card.group_number,
            card.station_number,
        );
        self.draw_competitor_info_table(&card.competitor);
        self.draw_attempt_table(card.attempt_count, card.time_limit_info);
    }

    /// Draws a blank scorecard for subsequent rounds.
    pub fn draw_blank_scorecard(&mut self, card: &BlankScorecard<'_>) {
        self.draw_outer_border();
        self.draw_top_header(card.number, &card.truncated_competition_name(30));
        self.draw_event_info_table(
            card.event_name(),
            card.round_number,
            card.group_number,
            card.station_number,
        );
        self.draw_blank_competitor_info_table();
        self.draw_attempt_table(card.attempt_count, card.time_limit_info);
    }

    /// Draws a complete cover sheet for a group with competition info, checkboxes, and signature fields.
    pub fn draw_cover_sheet(&mut self, card: &CoverSheet<'_>) {
        self.draw_outer_border();
        self.draw_cover_sheet_header(card);
        self.draw_delegate_section(card.total_group_cards);
        self.draw_data_entry_section();
    }

    fn draw_cover_sheet_header(&mut self, card: &CoverSheet<'_>) {
        self.advance_y(6.0);
        self.draw_full_width(card.competition_name, self.cur_y, 11.5, true);

        self.advance_y(14.0);
        let mut event_round_str = String::with_capacity(32);
        let _ = write!(
            event_round_str,
            "{} Round {}",
            card.event_name(),
            card.round_number
        );
        self.draw_full_width(&event_round_str, self.cur_y, 10.0, true);

        let mut group_stage_str = String::with_capacity(32);
        match (card.group_number, card.stage_name) {
            (g, Some(stage)) if g > 0 => {
                let _ = write!(group_stage_str, "Group {g} ({stage})");
            }
            (g, None) if g > 0 => {
                let _ = write!(group_stage_str, "Group {g}");
            }
            (0, Some(stage)) => {
                let _ = write!(group_stage_str, "Stage: {stage}");
            }
            _ => {}
        }

        if !group_stage_str.is_empty() {
            self.advance_y(13.0);
            self.draw_full_width(&group_stage_str, self.cur_y, 9.5, true);
        }
    }

    fn draw_delegate_section(&mut self, total_cards: usize) {
        self.advance_y(14.0);
        self.draw_section_banner("FOR DELEGATE");

        self.advance_y(14.0);
        let mut bundle_str = String::with_capacity(36);
        let _ = write!(bundle_str, "1. Bundled all {total_cards} scorecards");
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
    pub fn draw_card(
        ops: &mut Vec<Op>,
        card: &ScorecardItem<'_>,
        bounds: RectSpec,
        custom_font: Option<&FontId>,
    ) {
        let theme = DEFAULT_THEME.with_font(custom_font.cloned());
        let mut painter = CardPainter::new(ops, bounds, &theme);
        match card {
            ScorecardItem::Empty => {}
            ScorecardItem::CoverSheet(cover) => painter.draw_cover_sheet(cover),
            ScorecardItem::Scorecard(scorecard) => painter.draw_scorecard(scorecard),
            ScorecardItem::Blank(blank) => painter.draw_blank_scorecard(blank),
        }
    }
}

/// Helper struct for drawing grid tables with shaded headers and borders.
pub struct TableDrawer;

impl TableDrawer {
    /// Draws a styled grid table, advancing `cur_y` to the bottom of the table.
    pub fn draw(ops: &mut Vec<Op>, cur_y: &mut f32, spec: TableSpec<'_>, theme: &ScorecardTheme) {
        let top_y = *cur_y;
        let row_count = f32::from(u16::try_from(spec.rows.len()).unwrap_or(0));
        let total_h = spec.header_h + spec.row_h * row_count;
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
        Self::draw_rect(
            ops,
            RectSpec::new(spec.tbl_x, top_y - spec.header_h, spec.tbl_w, spec.header_h),
            PaintMode::Fill,
        );
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
            if !col.header.is_empty() {
                TextDrawer::draw(
                    ops,
                    TextSpec {
                        text: col.header,
                        cell_x: col_x,
                        baseline_y: text_y,
                        cell_w: w,
                        font_size: theme.header_font_size,
                        bold: false,
                        align: col.align,
                    },
                );
            }
            col_x += w;
        }
    }

    fn draw_row_cells(ops: &mut Vec<Op>, spec: &TableSpec<'_>, top_y: f32, theme: &ScorecardTheme) {
        let mut row_top = top_y - spec.header_h;
        for &row in spec.rows {
            let mut cell_x = spec.tbl_x;
            for (i, &cell) in row.iter().enumerate() {
                let col = spec.columns.get(i);
                let w = col.map_or(0.0, |c| c.ratio * spec.tbl_w);
                let align = col.map_or(TextAlign::Center, |c| c.align);
                let bold = col.is_some_and(|c| c.bold);
                let text_w = TextDrawer::estimate_width(cell, theme.cell_font_size, bold);
                let max_w = (w - 6.0).max(10.0);
                let font_size = if text_w > max_w {
                    (theme.cell_font_size * (max_w / text_w)).max(6.0)
                } else {
                    theme.cell_font_size
                };
                let text_y = row_top - spec.row_h + (spec.row_h - font_size) / 2.0 + 1.0;
                TextDrawer::draw(
                    ops,
                    TextSpec {
                        text: cell,
                        cell_x,
                        baseline_y: text_y,
                        cell_w: w,
                        font_size,
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
        Self::draw_rect(
            ops,
            RectSpec::new(spec.tbl_x, bottom_y, spec.tbl_w, total_h),
            PaintMode::Stroke,
        );
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
        for col in spec
            .columns
            .iter()
            .take(spec.columns.len().saturating_sub(1))
        {
            sep_x += col.ratio * spec.tbl_w;
            Self::draw_line(ops, sep_x, top_y, sep_x, bottom_y);
        }
    }

    /// Draws a styled rectangle primitive (fill or stroke) using `RectSpec` geometry.
    pub fn draw_rect(ops: &mut Vec<Op>, rect: RectSpec, mode: PaintMode) {
        ops.push(Op::DrawRectangle {
            rectangle: Rect {
                x: Pt(rect.x),
                y: Pt(rect.y),
                width: Pt(rect.w),
                height: Pt(rect.h),
                mode: Some(mode),
                winding_order: None,
            },
        });
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

const HELVETICA_WIDTHS: [f32; 95] = [
    0.278, 0.278, 0.355, 0.556, 0.556, 0.889, 0.667, 0.191, 0.333, 0.333, 0.389, 0.584, 0.278,
    0.333, 0.278, 0.278, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556,
    0.278, 0.278, 0.584, 0.584, 0.584, 0.556, 1.015, 0.667, 0.667, 0.722, 0.722, 0.667, 0.611,
    0.778, 0.722, 0.278, 0.500, 0.667, 0.556, 0.833, 0.722, 0.778, 0.667, 0.778, 0.722, 0.667,
    0.611, 0.722, 0.667, 0.944, 0.667, 0.667, 0.611, 0.278, 0.278, 0.278, 0.469, 0.556, 0.333,
    0.556, 0.556, 0.500, 0.556, 0.556, 0.278, 0.556, 0.556, 0.222, 0.222, 0.500, 0.222, 0.833,
    0.556, 0.556, 0.556, 0.556, 0.333, 0.500, 0.278, 0.556, 0.500, 0.722, 0.500, 0.500, 0.500,
    0.334, 0.260, 0.334, 0.584,
];

const HELVETICA_BOLD_WIDTHS: [f32; 95] = [
    0.278, 0.333, 0.474, 0.556, 0.556, 0.889, 0.722, 0.238, 0.333, 0.333, 0.389, 0.584, 0.278,
    0.333, 0.278, 0.278, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556,
    0.333, 0.333, 0.584, 0.584, 0.584, 0.611, 0.975, 0.722, 0.722, 0.722, 0.722, 0.667, 0.611,
    0.778, 0.722, 0.278, 0.556, 0.722, 0.611, 0.833, 0.722, 0.778, 0.667, 0.778, 0.722, 0.667,
    0.611, 0.722, 0.667, 0.944, 0.667, 0.667, 0.611, 0.333, 0.278, 0.333, 0.584, 0.556, 0.333,
    0.556, 0.611, 0.556, 0.611, 0.556, 0.333, 0.611, 0.611, 0.278, 0.278, 0.556, 0.278, 0.889,
    0.611, 0.611, 0.611, 0.611, 0.389, 0.556, 0.333, 0.611, 0.556, 0.778, 0.556, 0.556, 0.500,
    0.389, 0.280, 0.389, 0.584,
];

/// Helper struct for rendering standard text with Helvetica proportional font metrics and horizontal alignment.
pub struct TextDrawer;

impl TextDrawer {
    /// Subtle breathing space (in em) between unbolded parentheses and enclosed native/CJK characters.
    pub const CJK_PAREN_PAD_EM: f32 = 0.10;

    pub fn draw(ops: &mut Vec<Op>, spec: TextSpec<'_>) {
        if spec.text.is_empty() {
            return;
        }

        let text_w = Self::estimate_width(spec.text, spec.font_size, spec.bold);
        let cur_x = Self::compute_aligned_x(spec.align, spec.cell_x, spec.cell_w, text_w);
        let font = Self::resolve_font(spec.bold);

        Self::emit_text_ops(ops, font, spec.font_size, cur_x, spec.baseline_y, spec.text);
    }

    /// Renders a competitor name: bold Latin primary name followed optionally by unbolded parenthesized local name.
    pub fn draw_competitor_name(
        ops: &mut Vec<Op>,
        primary: &str,
        local: Option<&str>,
        start_x: f32,
        baseline_y: f32,
        font_size: f32,
        custom_font: Option<&FontId>,
    ) {
        let mut cur_x = start_x;

        if !primary.is_empty() {
            let font = if primary.chars().any(|c| !is_win_ansi(c)) {
                if let Some(id) = custom_font {
                    PdfFontHandle::External(id.clone())
                } else {
                    PdfFontHandle::Builtin(BuiltinFont::HelveticaBold)
                }
            } else {
                PdfFontHandle::Builtin(BuiltinFont::HelveticaBold)
            };
            Self::emit_text_ops(ops, font, font_size, cur_x, baseline_y, primary);
            cur_x += Self::estimate_width(primary, font_size, true);
        }

        if let Some(local_name) = local {
            let paren_pad = Self::CJK_PAREN_PAD_EM * font_size;

            let open_paren_str = if primary.is_empty() { "(" } else { " (" };
            Self::emit_text_ops(
                ops,
                PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                font_size,
                cur_x,
                baseline_y,
                open_paren_str,
            );
            cur_x += Self::estimate_width(open_paren_str, font_size, false) + paren_pad;

            let local_font = if let Some(id) = custom_font {
                PdfFontHandle::External(id.clone())
            } else {
                PdfFontHandle::Builtin(BuiltinFont::Helvetica)
            };
            Self::emit_text_ops(ops, local_font, font_size, cur_x, baseline_y, local_name);
            cur_x += Self::estimate_cjk_width(local_name, font_size) + paren_pad;

            Self::emit_text_ops(
                ops,
                PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                font_size,
                cur_x,
                baseline_y,
                ")",
            );
        }
    }

    /// Calculates total width needed to render a competitor name with optional local name and unbolded parentheses.
    pub fn estimate_competitor_name_width(
        primary: &str,
        local: Option<&str>,
        font_size: f32,
    ) -> f32 {
        let mut total = if primary.is_empty() {
            0.0
        } else {
            Self::estimate_width(primary, font_size, true)
        };

        if let Some(local_name) = local {
            let open_str = if primary.is_empty() { "(" } else { " (" };
            total += Self::estimate_width(open_str, font_size, false);
            total += Self::CJK_PAREN_PAD_EM * font_size;
            total += Self::estimate_cjk_width(local_name, font_size);
            total += Self::CJK_PAREN_PAD_EM * font_size;
            total += Self::estimate_width(")", font_size, false);
        }

        total
    }

    /// Measures width of CJK/native string using 1.0em for full-width CJK characters.
    pub fn estimate_cjk_width(text: &str, font_size: f32) -> f32 {
        text.chars()
            .map(|c| if is_cjk(c) { 1.0 } else { 0.50 })
            .sum::<f32>()
            * font_size
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

    pub fn emit_text_ops(
        ops: &mut Vec<Op>,
        font: PdfFontHandle,
        font_size: f32,
        x: f32,
        y: f32,
        text: &str,
    ) {
        ops.extend([
            Op::SetFillColor { col: grey(0.0) },
            Op::StartTextSection,
            Op::SetFont {
                font,
                size: Pt(font_size),
            },
            Op::SetTextCursor {
                pos: Point { x: Pt(x), y: Pt(y) },
            },
            Op::ShowText {
                items: vec![TextItem::Text(text.to_string())],
            },
            Op::EndTextSection,
        ]);
    }

    fn raw_str_width(text: &str, font_size: f32, bold: bool) -> f32 {
        text.chars()
            .map(|ch| Self::char_raw_width(ch, bold))
            .sum::<f32>()
            * font_size
    }

    /// Approximates proportional Helvetica font character widths.
    pub fn estimate_width(text: &str, font_size: f32, bold: bool) -> f32 {
        Self::raw_str_width(text, font_size, bold)
    }

    fn char_raw_width(ch: char, bold: bool) -> f32 {
        let code = u32::from(ch);
        if (32..=126).contains(&code) {
            let idx = usize::try_from(code - 32).unwrap_or(0);
            if bold {
                HELVETICA_BOLD_WIDTHS[idx]
            } else {
                HELVETICA_WIDTHS[idx]
            }
        } else if is_cjk(ch) {
            1.0
        } else {
            0.50
        }
    }
}

/// Returns true if the character is in CJK Unicode blocks (ideographs, Hangul, Kana).
#[inline]
pub fn is_cjk(ch: char) -> bool {
    matches!(u32::from(ch),
        0x4E00..=0x9FFF |
        0x3400..=0x4DBF |
        0x20000..=0x2A6DF |
        0x2A700..=0x2B73F |
        0x2B740..=0x2B81F |
        0x2B820..=0x2CEAF |
        0xF900..=0xFAFF |
        0xAC00..=0xD7AF |
        0x1100..=0x11FF |
        0x3130..=0x318F |
        0x3040..=0x309F |
        0x30A0..=0x30FF |
        0x3000..=0x303F |
        0xFF01..=0xFF60 |
        0xFFE0..=0xFFE6
    )
}

/// Returns true if the character is directly encodable in standard PDF `WinAnsiEncoding`.
#[inline]
pub fn is_win_ansi(c: char) -> bool {
    match u32::from(c) {
        0x20..=0x7E | 0xA0..=0xFF => true,
        _ => matches!(
            c,
            '\u{20AC}'
                | '\u{201A}'
                | '\u{0192}'
                | '\u{201E}'
                | '\u{2026}'
                | '\u{2020}'
                | '\u{2021}'
                | '\u{02C6}'
                | '\u{2030}'
                | '\u{0160}'
                | '\u{2039}'
                | '\u{0152}'
                | '\u{017D}'
                | '\u{2018}'
                | '\u{2019}'
                | '\u{201C}'
                | '\u{201D}'
                | '\u{2022}'
                | '\u{2013}'
                | '\u{2014}'
                | '\u{02DC}'
                | '\u{2122}'
                | '\u{0161}'
                | '\u{203A}'
                | '\u{0153}'
                | '\u{017E}'
                | '\u{0178}'
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scorecard::{Competitor, ScorecardItem, TimeLimitInfo, WcaEvent, WcaId, WcaResult};

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
        let card = ScorecardItem::scorecard(
            "Test Comp 2026",
            WcaEvent::E333,
            1,
            1,
            Some("Red Stage"),
            Competitor {
                name: "Alice Smith",
                local_name: None,
                registrant_id: std::num::NonZeroUsize::MIN,
                wca_id: WcaId::parse("2022SMIT01"),
            },
            Some(4),
            5,
            Some(TimeLimitInfo {
                limit_centiseconds: WcaResult::new(60000),
                is_cumulative: false,
                cutoff_centiseconds: None,
                cutoff_attempts: 0,
            }),
        );

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
        let cover_card = ScorecardItem::cover_sheet(
            "Ocean State Cubikon 2025",
            WcaEvent::E333,
            1,
            1,
            Some("Main Hall"),
            15,
        );

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
        let theme = DEFAULT_THEME;
        assert_eq!(theme.padding, 7.0);
        assert_eq!(theme.border_thickness, 0.75);
        assert!(theme.title_font_size > theme.header_font_size);
    }

    #[test]
    fn test_event_info_table_3_cols_without_station() {
        let card_no_station = ScorecardItem::scorecard(
            "Test Comp 2026",
            WcaEvent::E333,
            1,
            1,
            None,
            Competitor::simple("Alice Smith"),
            None,
            5,
            None,
        );

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

        let card_with_station = ScorecardItem::scorecard(
            "Test Comp 2026",
            WcaEvent::E333,
            1,
            1,
            None,
            Competitor::simple("Alice Smith"),
            Some(3),
            5,
            None,
        );
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
        let card = ScorecardItem::scorecard(
            "Test Comp 2026",
            WcaEvent::E333,
            1,
            1,
            None,
            Competitor::simple("Alice Smith"),
            None,
            5,
            Some(TimeLimitInfo {
                limit_centiseconds: WcaResult::new(60000),
                is_cumulative: false,
                cutoff_centiseconds: WcaResult::new(4500),
                cutoff_attempts: 2,
            }),
        );

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
        let blank_card =
            ScorecardItem::blank("Test Comp 2026", WcaEvent::E333, 2, 1, None, 5, None);

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
        let custom_font = FontId("CustomFontTest".to_string());
        TextDrawer::draw_competitor_name(
            &mut ops,
            "Marco Yang",
            Some("杨柯辰"),
            13.0,
            100.0,
            10.0,
            Some(&custom_font),
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
                printpdf::ops::PdfFontHandle::External(FontId("CustomFontTest".to_string())),
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

        // 1. Initial position is 13.0
        assert!((x_latin - 13.0).abs() < 1e-4);

        // 2. Open paren starts after "Marco Yang"
        assert!(x_open > x_latin);

        // 3. CJK text starts after " (" PLUS paren_pad (0.10 * 10.0 = 1.0 pt)
        let open_paren_w = TextDrawer::estimate_width(" (", 10.0, false);
        assert!((x_cjk - (x_open + open_paren_w + 1.0)).abs() < 1e-4);

        // 4. Close paren starts after "杨柯辰" (3 * 10.0 = 30.0 pt) PLUS paren_pad (1.0 pt)
        assert!((x_close - (x_cjk + 30.0 + 1.0)).abs() < 1e-4);
    }
}
