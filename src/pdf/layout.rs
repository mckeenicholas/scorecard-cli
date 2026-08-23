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
            (page_w_pt - 2.0 * A6_MARGIN, page_h_pt - 2.0 * A6_MARGIN)
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
            RectSpec {
                x: A6_MARGIN,
                y: A6_MARGIN,
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
