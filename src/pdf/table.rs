use printpdf::graphics::{Line, LinePoint, PaintMode, Point, Rect};
use printpdf::ops::Op;
use printpdf::units::Pt;

use crate::pdf::layout::RectSpec;
use crate::pdf::text::{self, TextAlign, TextDrawer, TextSpec};
use crate::pdf::theme::ScorecardTheme;

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
            col: text::grey(theme.header_bg_grey),
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
            col: text::grey(theme.grid_line_grey),
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
