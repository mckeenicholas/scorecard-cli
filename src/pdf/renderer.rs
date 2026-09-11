use std::fmt::Write as _;

use printpdf::FontId;
use printpdf::graphics::PaintMode;
use printpdf::ops::Op;
use printpdf::units::Pt;

use crate::pdf::attempt::{self, AttemptTableSpec};
use crate::pdf::layout::RectSpec;
use crate::pdf::table::{ColumnDef, TableDrawer, TableSpec};
use crate::pdf::text::{self, CompetitorNameSpec, TextAlign, TextDrawer, TextSpec};
use crate::pdf::theme::{self, ScorecardTheme};
use crate::scorecard::{
    BlankScorecard, Competitor, CoverSheet, GroupNumber, RoundNumber, Scorecard, ScorecardItem,
    TimeLimitInfo,
};

/// Canvas abstraction managing vertical flow, bounding geometry, and rendering primitives for a scorecard.
pub struct CardPainter<'a> {
    pub ops: &'a mut Vec<Op>,
    pub theme: ScorecardTheme<'a>,
    pub bounds: RectSpec,
    pub inner_x: f32,
    pub inner_w: f32,
    pub cur_y: f32,
    pub min_y: f32,
}

impl<'a> CardPainter<'a> {
    pub fn new(ops: &'a mut Vec<Op>, bounds: RectSpec, theme: ScorecardTheme<'a>) -> Self {
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
            col: text::grey(val),
        });
        self.ops.push(Op::SetOutlineThickness { pt: Pt(thickness) });
    }

    #[inline]
    pub fn set_fill(&mut self, val: f32) {
        self.ops.push(Op::SetFillColor {
            col: text::grey(val),
        });
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
            &self.theme,
        );
    }

    /// Draws the event, round, group, and station info grid table.
    /// If there is no station number (or station numbers are disabled), renders a 3-column table.
    pub fn draw_event_info_table(
        &mut self,
        event_name: &str,
        round_number: RoundNumber,
        group_number: GroupNumber,
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
            let cell = RectSpec::new(
                self.inner_x + 0.16 * self.inner_w,
                row_top,
                0.54 * self.inner_w,
                20.0,
            );
            self.draw_competitor_name(competitor.name, competitor.local_name, cell);
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

    fn draw_competitor_name(&mut self, primary: &str, local: Option<&str>, cell: RectSpec) {
        let max_w = (cell.w - 6.0).max(10.0);
        let full_w =
            TextDrawer::estimate_competitor_name_width(primary, local, self.theme.cell_font_size);
        let font_size = if full_w > max_w {
            (self.theme.cell_font_size * (max_w / full_w)).max(6.0)
        } else {
            self.theme.cell_font_size
        };

        let baseline_y = cell.y - cell.h + (cell.h - font_size) / 2.0 + 1.0;
        let pad = 3.0;
        let start_x = cell.x + pad;

        TextDrawer::draw_competitor_name(
            self.ops,
            CompetitorNameSpec {
                primary,
                local,
                start_x,
                baseline_y,
                font_size,
                custom_font: self.theme.custom_font,
            },
        );
    }

    /// Draws the attempt table dynamically sized to fill the remaining scorecard height,
    /// with inline cutoff banner, extra solve banner, blank extra box, and bottom time limit footer.
    pub fn draw_attempt_table(
        &mut self,
        attempt_count: usize,
        time_limit_info: Option<TimeLimitInfo>,
        has_checker: bool,
    ) {
        self.advance_y(5.0);
        let spec = AttemptTableSpec::build(
            self.inner_w,
            self.cur_y - self.min_y,
            attempt_count,
            time_limit_info,
            has_checker,
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
        for (i, col) in spec.columns().iter().enumerate() {
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
                attempt::ATTEMPT_LABELS[i - 1]
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
        for &w in spec.active_col_widths().iter().take(spec.col_count - 1) {
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

    /// Draws a text label followed by a square checkbox on cover sheets (centered).
    pub fn draw_checkbox_item(&mut self, text: &str) {
        let box_size = 8.0f32;
        let gap = 6.0f32;
        let text_w = TextDrawer::estimate_width(text, 8.5, false);
        let total_w = text_w + gap + box_size;
        let start_x = self.inner_x + (self.inner_w - total_w) / 2.0;
        let box_y = self.cur_y - 1.0;

        TextDrawer::draw(
            self.ops,
            TextSpec {
                text,
                cell_x: start_x,
                baseline_y: self.cur_y,
                cell_w: text_w + 2.0,
                font_size: 8.5,
                bold: false,
                align: TextAlign::Left,
            },
        );

        let box_x = start_x + text_w + gap;
        self.set_outline(0.2, 0.75);
        self.draw_stroked_rect(RectSpec::new(box_x, box_y, box_size, box_size));
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
        self.draw_attempt_table(
            card.attempt_count,
            card.time_limit_info,
            card.needs_scramble_checker,
        );
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
        self.draw_attempt_table(
            card.attempt_count,
            card.time_limit_info,
            card.needs_scramble_checker,
        );
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
        let theme = theme::DEFAULT_THEME.with_font(custom_font);
        let mut painter = CardPainter::new(ops, bounds, theme);
        match card {
            ScorecardItem::Empty => {}
            ScorecardItem::CoverSheet(cover) => painter.draw_cover_sheet(cover),
            ScorecardItem::Scorecard(scorecard) => painter.draw_scorecard(scorecard),
            ScorecardItem::Blank(blank) => painter.draw_blank_scorecard(blank),
        }
    }
}

#[cfg(test)]
#[path = "renderer_tests.rs"]
mod tests;
