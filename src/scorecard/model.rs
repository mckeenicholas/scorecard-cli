use std::borrow::Cow;

/// ScorecardItem contains the data needed to render a single scorecard without heap allocations.
#[derive(Debug, Clone)]
pub struct ScorecardItem<'a> {
    pub scorecard_number: usize,
    pub station_number: Option<usize>,
    pub competition_name: &'a str,
    #[allow(dead_code)] // Stored for future cover sheet / event-specific formatting use
    pub event_id: &'a str,
    pub event_name: &'static str,
    pub round_number: usize,
    pub group_number: usize,
    pub competitor_name: &'a str,
    pub registrant_id: Option<usize>,
    pub wca_id: Option<&'a str>,
    pub attempt_count: usize,
    pub is_blank: bool,
}

impl<'a> ScorecardItem<'a> {
    /// Returns formatted competitor name, including WCA ID or new competitor marker.
    pub fn display_competitor_name(&self) -> Cow<'_, str> {
        if self.is_blank {
            Cow::Borrowed("[ Blank Scorecard ]")
        } else if let Some(wca_id) = self.wca_id {
            if !wca_id.is_empty() {
                Cow::Owned(format!("{} ({})", self.competitor_name, wca_id))
            } else {
                Cow::Owned(format!("{} (New Competitor)", self.competitor_name))
            }
        } else {
            Cow::Owned(format!("{} (New Competitor)", self.competitor_name))
        }
    }

    /// Returns truncated competition name if exceeding max_chars.
    pub fn truncated_competition_name(&self, max_chars: usize) -> Cow<'_, str> {
        if self.competition_name.chars().count() > max_chars {
            let truncated: String = self
                .competition_name
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect();
            Cow::Owned(format!("{}...", truncated))
        } else {
            Cow::Borrowed(self.competition_name)
        }
    }
}
