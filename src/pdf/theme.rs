use printpdf::FontId;

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
