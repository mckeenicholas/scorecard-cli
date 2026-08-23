use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Supported paper sizes for scorecard printing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaperSize {
    #[default]
    #[value(name = "a4", alias = "A4")]
    A4,
    #[value(name = "letter", alias = "Letter")]
    Letter,
    #[value(name = "a6", alias = "A6")]
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

/// Page layout format for scorecards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageFormat {
    #[default]
    #[value(name = "group")]
    Group,
    #[value(name = "stacked")]
    Stacked,
}

impl FromStr for PageFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "group" => Ok(PageFormat::Group),
            "stacked" => Ok(PageFormat::Stacked),
            other => Err(format!(
                "invalid format '{}': must be 'group' or 'stacked'",
                other
            )),
        }
    }
}

impl fmt::Display for PageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PageFormat::Group => write!(f, "group"),
            PageFormat::Stacked => write!(f, "stacked"),
        }
    }
}

/// Margin used for A6 single-card layouts (in PDF points).
const A6_MARGIN: f32 = 14.0;

/// Default page margin for multi-card grid layouts (in PDF points).
const DEFAULT_GRID_MARGIN: f32 = 18.0;

/// Default gap between cards in multi-card grid layouts (in PDF points).
const DEFAULT_GRID_GAP: f32 = 12.0;

impl PaperSize {
    const A6_MM: (f32, f32) = (105.0, 148.0);
    const A4_MM: (f32, f32) = (210.0, 297.0);
    const LETTER_MM: (f32, f32) = (215.9, 279.4);

    const MM_TO_PT: f32 = 2.834_645_7;

    /// Physical paper dimensions in millimeters (width, height).
    pub fn dimensions_mm(&self) -> (f32, f32) {
        match self {
            PaperSize::A6 => Self::A6_MM,
            PaperSize::A4 => Self::A4_MM,
            PaperSize::Letter => Self::LETTER_MM,
        }
    }

    fn to_ps(dimensions: (f32, f32)) -> (f32, f32) {
        (dimensions.0 * Self::MM_TO_PT, dimensions.1 * Self::MM_TO_PT)
    }

    /// Physical paper dimensions in PostScript points (width, height).
    pub fn dimensions_pt(&self) -> (f32, f32) {
        Self::to_ps(self.dimensions_mm())
    }

    /// Number of scorecard cards printable per sheet for this paper size.
    pub fn cards_per_page(&self) -> usize {
        match self {
            PaperSize::A6 => 1,
            PaperSize::A4 | PaperSize::Letter => 4,
        }
    }
}

/// Spacing parameters (margins and gaps) for a scorecard page layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSpacing {
    pub margin_x: f32,
    pub margin_y: f32,
    pub gap_x: f32,
    pub gap_y: f32,
}

impl PageSpacing {
    /// Resolves standard margins and gaps for the specified paper size.
    pub fn for_paper_size(paper_size: PaperSize) -> Self {
        if paper_size.cards_per_page() == 1 {
            Self {
                margin_x: A6_MARGIN,
                margin_y: A6_MARGIN,
                gap_x: 0.0,
                gap_y: 0.0,
            }
        } else {
            Self {
                margin_x: DEFAULT_GRID_MARGIN,
                margin_y: DEFAULT_GRID_MARGIN,
                gap_x: DEFAULT_GRID_GAP,
                gap_y: DEFAULT_GRID_GAP,
            }
        }
    }
}

/// Computes available width and height for each card given sheet dimensions and spacing.
fn compute_card_dimensions(
    page_w_pt: f32,
    page_h_pt: f32,
    cards_per_page: usize,
    spacing: &PageSpacing,
) -> (f32, f32) {
    if cards_per_page == 1 {
        (
            page_w_pt - 2.0 * spacing.margin_x,
            page_h_pt - 2.0 * spacing.margin_y,
        )
    } else {
        (
            (page_w_pt - 2.0 * spacing.margin_x - spacing.gap_x) / 2.0,
            (page_h_pt - 2.0 * spacing.margin_y - spacing.gap_y) / 2.0,
        )
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
    /// Creates a PageLayout configuration from a PaperSize enum.
    pub fn new(paper_size: PaperSize) -> Self {
        let (page_w_mm, page_h_mm) = paper_size.dimensions_mm();
        let (page_w_pt, page_h_pt) = paper_size.dimensions_pt();
        let cards_per_page = paper_size.cards_per_page();
        let spacing = PageSpacing::for_paper_size(paper_size);
        let (card_w, card_h) =
            compute_card_dimensions(page_w_pt, page_h_pt, cards_per_page, &spacing);

        Self {
            paper_size,
            page_w_mm,
            page_h_mm,
            page_w_pt,
            page_h_pt,
            cards_per_page,
            margin_x: spacing.margin_x,
            margin_y: spacing.margin_y,
            gap_x: spacing.gap_x,
            gap_y: spacing.gap_y,
            card_w,
            card_h,
        }
    }

    /// Returns the bounding box rectangle for the card at index `idx` on the current page (0..cards_per_page).
    pub fn card_rect(&self, idx: usize) -> RectSpec {
        if self.cards_per_page == 1 {
            self.single_card_rect()
        } else {
            self.grid_card_rect(idx)
        }
    }

    fn single_card_rect(&self) -> RectSpec {
        RectSpec {
            x: self.margin_x,
            y: self.margin_y,
            w: self.card_w,
            h: self.card_h,
        }
    }

    fn grid_card_rect(&self, idx: usize) -> RectSpec {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paper_size_parsing_and_display() {
        assert_eq!("a4".parse::<PaperSize>(), Ok(PaperSize::A4));
        assert_eq!("A4".parse::<PaperSize>(), Ok(PaperSize::A4));
        assert_eq!("letter".parse::<PaperSize>(), Ok(PaperSize::Letter));
        assert_eq!("Letter".parse::<PaperSize>(), Ok(PaperSize::Letter));
        assert_eq!("a6".parse::<PaperSize>(), Ok(PaperSize::A6));
        assert!("tabloid".parse::<PaperSize>().is_err());

        assert_eq!(PaperSize::A4.to_string(), "A4");
        assert_eq!(PaperSize::Letter.to_string(), "Letter");
        assert_eq!(PaperSize::A6.to_string(), "A6");
    }

    #[test]
    fn test_page_format_parsing_and_display() {
        assert_eq!("group".parse::<PageFormat>(), Ok(PageFormat::Group));
        assert_eq!("Group".parse::<PageFormat>(), Ok(PageFormat::Group));
        assert_eq!("stacked".parse::<PageFormat>(), Ok(PageFormat::Stacked));
        assert_eq!("Stacked".parse::<PageFormat>(), Ok(PageFormat::Stacked));
        assert!("invalid".parse::<PageFormat>().is_err());

        assert_eq!(PageFormat::Group.to_string(), "group");
        assert_eq!(PageFormat::Stacked.to_string(), "stacked");
    }

    #[test]
    fn test_layout_a4_quadrants() {
        let layout = PageLayout::new(PaperSize::A4);
        assert_eq!(layout.cards_per_page, 4);
        assert!(layout.card_w > 200.0);
        assert!(layout.card_h > 300.0);

        let r0 = layout.card_rect(0); // Top-left
        let r1 = layout.card_rect(1); // Top-right
        let r2 = layout.card_rect(2); // Bottom-left
        let r3 = layout.card_rect(3); // Bottom-right

        // Top quadrants should have higher y than bottom quadrants
        assert!(r0.y > r2.y);
        assert!(r1.y > r3.y);
        // Right quadrants should have higher x than left quadrants
        assert!(r1.x > r0.x);
        assert!(r3.x > r2.x);
    }

    #[test]
    fn test_layout_a6_single_card() {
        let layout = PageLayout::new(PaperSize::A6);
        assert_eq!(layout.cards_per_page, 1);
        let r = layout.card_rect(0);
        assert_eq!(r.x, 14.0);
        assert_eq!(r.y, 14.0);
        assert_eq!(r.w, layout.page_w_pt - 28.0);
        assert_eq!(r.h, layout.page_h_pt - 28.0);
    }
}
