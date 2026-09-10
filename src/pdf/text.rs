use printpdf::FontId;
use printpdf::color::{Color, Greyscale};
use printpdf::font::BuiltinFont;
use printpdf::graphics::Point;
use printpdf::ops::{Op, PdfFontHandle};
use printpdf::text::TextItem;
use printpdf::units::Pt;

#[inline]
pub(crate) fn grey(v: f32) -> Color {
    Color::Greyscale(Greyscale::new(v, None))
}

/// Text alignment within a scorecard cell or bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
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

pub const HELVETICA_WIDTHS: [f32; 95] = [
    0.278, 0.278, 0.355, 0.556, 0.556, 0.889, 0.667, 0.191, 0.333, 0.333, 0.389, 0.584, 0.278,
    0.333, 0.278, 0.278, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556, 0.556,
    0.278, 0.278, 0.584, 0.584, 0.584, 0.556, 1.015, 0.667, 0.667, 0.722, 0.722, 0.667, 0.611,
    0.778, 0.722, 0.278, 0.500, 0.667, 0.556, 0.833, 0.722, 0.778, 0.667, 0.778, 0.722, 0.667,
    0.611, 0.722, 0.667, 0.944, 0.667, 0.667, 0.611, 0.278, 0.278, 0.278, 0.469, 0.556, 0.333,
    0.556, 0.556, 0.500, 0.556, 0.556, 0.278, 0.556, 0.556, 0.222, 0.222, 0.500, 0.222, 0.833,
    0.556, 0.556, 0.556, 0.556, 0.333, 0.500, 0.278, 0.556, 0.500, 0.722, 0.500, 0.500, 0.500,
    0.334, 0.260, 0.334, 0.584,
];

pub const HELVETICA_BOLD_WIDTHS: [f32; 95] = [
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
