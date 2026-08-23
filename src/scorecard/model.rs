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
    pub stage_name: Option<&'a str>,
    pub competitor_name: &'a str,
    pub registrant_id: Option<usize>,
    pub wca_id: Option<&'a str>,
    pub attempt_count: usize,
    /// Note: `time_limit_info` is cloned per card within a round. All cards in the same
    /// round share the same value, so an `Arc<str>` could avoid per-card allocations if
    /// performance on very large competitions becomes a concern.
    pub time_limit_info: Option<String>,
    pub is_blank: bool,
}

#[cfg(test)]
impl Default for ScorecardItem<'static> {
    fn default() -> Self {
        Self {
            scorecard_number: 1,
            station_number: Some(1),
            competition_name: "Test Comp",
            event_id: "333",
            event_name: "3x3x3 Cube",
            round_number: 1,
            group_number: 1,
            stage_name: Some("Main Stage"),
            competitor_name: "Alice",
            registrant_id: Some(1),
            wca_id: None,
            attempt_count: 5,
            time_limit_info: None,
            is_blank: false,
        }
    }
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

/// Summary information for a planned competition round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedRoundSummary {
    OpenRound {
        event_id: String,
        round_number: usize,
        competitor_count: usize,
        sample_competitor_names: Vec<String>,
    },
    SubsequentRound {
        event_id: String,
        round_number: usize,
        blank_count: usize,
        reason: String,
    },
}

/// ScorecardPlan contains all generated scorecard items along with round summaries and diagnostic notes.
#[derive(Debug, Clone, Default)]
pub struct ScorecardPlan<'a> {
    pub items: Vec<ScorecardItem<'a>>,
    pub summaries: Vec<PlannedRoundSummary>,
    pub notes: Vec<String>,
}

impl<'a> ScorecardPlan<'a> {
    pub fn new(notes: Vec<String>) -> Self {
        Self {
            items: Vec::new(),
            summaries: Vec::new(),
            notes,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn assign_scorecard_numbers(&mut self) {
        for (idx, card) in self.items.iter_mut().enumerate() {
            card.scorecard_number = idx + 1;
        }
    }

    /// Formats the plan summary as a readable string for console output.
    pub fn format_summary(&self) -> String {
        let mut out = String::new();
        for note in &self.notes {
            out.push_str(&format!("Note: {}\n", note));
        }
        out.push_str("\n--- Scorecard Generation Plan ---\n");
        for summary in &self.summaries {
            match summary {
                PlannedRoundSummary::OpenRound {
                    event_id,
                    round_number,
                    competitor_count,
                    sample_competitor_names,
                } => {
                    out.push_str(&format!(
                        "[{event_id} Round {round_number}] (Open Round) -> Generating scorecards for {competitor_count} accepted competitors\n"
                    ));
                    if *competitor_count <= 5 && *competitor_count > 0 {
                        out.push_str(&format!(
                            "   Competitors: {}\n",
                            sample_competitor_names.join(", ")
                        ));
                    }
                }
                PlannedRoundSummary::SubsequentRound {
                    event_id,
                    round_number,
                    blank_count,
                    reason,
                } => {
                    out.push_str(&format!(
                        "[{event_id} Round {round_number}] (Subsequent Round) -> Generating {blank_count} blank scorecards ({reason})\n"
                    ));
                }
            }
        }
        out.push_str("---------------------------------");
        out
    }

    /// Prints the plan summary to stdout.
    pub fn print_summary(&self) {
        println!("{}", self.format_summary());
    }
}

impl<'a> std::ops::Deref for ScorecardPlan<'a> {
    type Target = [ScorecardItem<'a>];

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

impl<'a> IntoIterator for ScorecardPlan<'a> {
    type Item = ScorecardItem<'a>;
    type IntoIter = std::vec::IntoIter<ScorecardItem<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, 'b> IntoIterator for &'b ScorecardPlan<'a> {
    type Item = &'b ScorecardItem<'a>;
    type IntoIter = std::slice::Iter<'b, ScorecardItem<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}
