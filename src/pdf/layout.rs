use std::fmt;
use std::str::FromStr;

/// Supported paper sizes for scorecard printing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaperSize {
    #[default]
    A4,
    Letter,
    A6,
}

impl FromStr for PaperSize {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "a4" => Ok(PaperSize::A4),
            "letter" => Ok(PaperSize::Letter),
            "a6" => Ok(PaperSize::A6),
            other => Err(format!(
                "invalid paper size '{}': must be one of 'a4', 'letter', or 'a6'",
                other
            )),
        }
    }
}

impl fmt::Display for PaperSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaperSize::A4 => write!(f, "A4"),
            PaperSize::Letter => write!(f, "Letter"),
            PaperSize::A6 => write!(f, "A6"),
        }
    }
}

/// Bounding rectangle in PDF points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectSpec {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// PageLayout encapsulates paper geometry and scorecard positioning grid math.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageLayout {
    pub paper_size: PaperSize,
    pub page_w_mm: f32,
    pub page_h_mm: f32,
    pub page_w_pt: f32,
    pub page_h_pt: f32,
    pub cards_per_page: usize,
    pub margin_x: f32,
    pub margin_y: f32,
    pub gap_x: f32,
    pub gap_y: f32,
    pub card_w: f32,
    pub card_h: f32,
}

impl PageLayout {
    /// Creates a PageLayout configuration from a paper size string.
    pub fn from_paper_size(paper_str: &str) -> Self {
        let paper = PaperSize::from_str(paper_str).unwrap_or_default();
        Self::new(paper)
    }

    /// Creates a PageLayout configuration from a PaperSize enum.
    pub fn new(paper_size: PaperSize) -> Self {
        let (page_w_mm, page_h_mm, page_w_pt, page_h_pt, cards_per_page) = match paper_size {
            PaperSize::A6 => (105.0, 148.0, 297.64, 419.53, 1),
            PaperSize::A4 => (210.0, 297.0, 595.28, 841.89, 4),
            PaperSize::Letter => (215.9, 279.4, 612.0, 792.0, 4),
        };

        let margin_x = 18.0;
        let margin_y = 18.0;
        let gap_x = 12.0;
        let gap_y = 12.0;

        let (card_w, card_h) = if cards_per_page == 1 {
            let margin_a6 = 14.0;
            (page_w_pt - 2.0 * margin_a6, page_h_pt - 2.0 * margin_a6)
        } else {
            (
                (page_w_pt - 2.0 * margin_x - gap_x) / 2.0,
                (page_h_pt - 2.0 * margin_y - gap_y) / 2.0,
            )
        };

        Self {
            paper_size,
            page_w_mm,
            page_h_mm,
            page_w_pt,
            page_h_pt,
            cards_per_page,
            margin_x,
            margin_y,
            gap_x,
            gap_y,
            card_w,
            card_h,
        }
    }

    /// Returns the bounding box rectangle for the card at index `idx` on the current page (0..cards_per_page).
    pub fn card_rect(&self, idx: usize) -> RectSpec {
        if self.cards_per_page == 1 {
            let margin_a6 = 14.0;
            RectSpec {
                x: margin_a6,
                y: margin_a6,
                w: self.card_w,
                h: self.card_h,
            }
        } else {
            // 2x2 grid positions in bottom-left PDF coordinate system:
            // 0: Top-Left, 1: Top-Right, 2: Bottom-Left, 3: Bottom-Right
            let (x, y) = match idx {
                0 => (self.margin_x, self.margin_y + self.card_h + self.gap_y),
                1 => (
                    self.margin_x + self.card_w + self.gap_x,
                    self.margin_y + self.card_h + self.gap_y,
                ),
                2 => (self.margin_x, self.margin_y),
                _ => (self.margin_x + self.card_w + self.gap_x, self.margin_y),
            };

            RectSpec {
                x,
                y,
                w: self.card_w,
                h: self.card_h,
            }
        }
    }
}
