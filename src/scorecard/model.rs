use super::events::WcaEvent;
use crate::wcif::{Cutoff, TimeLimit};
use std::borrow::Cow;
use std::fmt::Write as _;

/// Compact, Copy-able metadata about a round's time limit and cutoff.
/// Captures integer centiseconds and attempt counts without any heap allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimeLimitInfo {
    pub limit_centiseconds: Option<isize>,
    pub is_cumulative: bool,
    pub cutoff_centiseconds: Option<isize>,
    pub cutoff_attempts: usize,
}

impl TimeLimitInfo {
    /// Creates a `TimeLimitInfo` from optional WCIF `TimeLimit` and Cutoff objects.
    /// Returns `None` if neither a time limit nor a cutoff is present.
    pub fn from_wcif(time_limit: Option<&TimeLimit>, cutoff: Option<&Cutoff>) -> Option<Self> {
        if time_limit.is_none() && cutoff.is_none() {
            return None;
        }

        Some(Self {
            limit_centiseconds: time_limit.map(|tl| tl.centiseconds),
            is_cumulative: time_limit
                .and_then(|tl| tl.cumulative_round_ids.as_ref())
                .is_some_and(|ids| !ids.is_empty()),
            cutoff_centiseconds: cutoff.map(|c| c.attempt_result),
            cutoff_attempts: cutoff.map_or(0, |c| c.number_of_attempts),
        })
    }

    /// Formats centiseconds into a human-readable time string (e.g. "1:30.50").
    /// Returns "None" for values <= 0 (covers WCA sentinels: -1 = DNF, -2 = DNS).
    pub fn format_centiseconds(centis: isize) -> String {
        if centis <= 0 {
            return "None".to_string();
        }

        let total_seconds = centis / 100;
        let cs = centis % 100;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;

        if minutes > 0 {
            if cs > 0 {
                format!("{minutes}:{seconds:02}.{cs:02}")
            } else {
                format!("{minutes}:{seconds:02}.00")
            }
        } else {
            format!("{seconds}.{cs:02}")
        }
    }

    /// Formats the cutoff and time limit info into a display string for scorecard footers.
    pub fn format_display(&self) -> String {
        let mut s = String::new();
        if let Some(cs) = self.cutoff_centiseconds {
            let _ = write!(
                s,
                "Cutoff: < {} ({} att)",
                Self::format_centiseconds(cs),
                self.cutoff_attempts
            );
        }

        if let Some(cs) = self.limit_centiseconds {
            if !s.is_empty() {
                s.push_str("  |  ");
            }
            let time_str = Self::format_centiseconds(cs);
            if self.is_cumulative {
                let _ = write!(s, "Time limit: {time_str} cumulative");
            } else {
                let _ = write!(s, "Time limit: {time_str}");
            }
        }

        s
    }
}

/// `ScorecardItem` contains the data needed to render a single scorecard without heap allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScorecardItem<'a> {
    pub scorecard_number: usize,
    pub station_number: Option<usize>,
    pub competition_name: &'a str,
    pub event: WcaEvent,
    pub round_number: usize,
    pub group_number: usize,
    pub stage_name: Option<&'a str>,
    pub competitor_name: &'a str,
    pub registrant_id: Option<usize>,
    pub wca_id: Option<&'a str>,
    pub attempt_count: usize,
    pub time_limit_info: Option<TimeLimitInfo>,
    pub is_blank: bool,
    pub is_cover_sheet: bool,
    pub total_group_cards: usize,
}

#[cfg(test)]
impl Default for ScorecardItem<'static> {
    fn default() -> Self {
        Self {
            scorecard_number: 1,
            station_number: Some(1),
            competition_name: "Test Comp",
            event: WcaEvent::E333,
            round_number: 1,
            group_number: 1,
            stage_name: Some("Main Stage"),
            competitor_name: "Alice",
            registrant_id: Some(1),
            wca_id: None,
            attempt_count: 5,
            time_limit_info: None,
            is_blank: false,
            is_cover_sheet: false,
            total_group_cards: 0,
        }
    }
}

impl<'a> ScorecardItem<'a> {
    /// Returns the standard WCA event ID string (e.g. `"333"`).
    #[must_use]
    pub const fn event_id(&self) -> &'static str {
        self.event.code()
    }

    /// Returns the user-friendly display name of the event (e.g. `"3x3x3 Cube"`).
    #[must_use]
    pub const fn event_name(&self) -> &'static str {
        self.event.display_name()
    }

    /// Creates a competitor scorecard item for an open round.
    #[allow(clippy::too_many_arguments)]
    pub fn competitor(
        competition_name: &'a str,
        event: WcaEvent,
        round_number: usize,
        group_number: usize,
        stage_name: Option<&'a str>,
        competitor_name: &'a str,
        registrant_id: Option<usize>,
        wca_id: Option<&'a str>,
        station_number: Option<usize>,
        attempt_count: usize,
        time_limit_info: Option<TimeLimitInfo>,
    ) -> Self {
        Self {
            scorecard_number: 0,
            station_number,
            competition_name,
            event,
            round_number,
            group_number,
            stage_name,
            competitor_name,
            registrant_id,
            wca_id,
            attempt_count,
            time_limit_info,
            is_blank: false,
            is_cover_sheet: false,
            total_group_cards: 0,
        }
    }

    /// Creates a cover sheet item to precede a group's scorecards.
    #[allow(clippy::too_many_arguments)]
    pub fn cover_sheet(
        competition_name: &'a str,
        event: WcaEvent,
        round_number: usize,
        group_number: usize,
        stage_name: Option<&'a str>,
        attempt_count: usize,
        total_group_cards: usize,
    ) -> Self {
        Self {
            scorecard_number: 0,
            station_number: None,
            competition_name,
            event,
            round_number,
            group_number,
            stage_name,
            competitor_name: "",
            registrant_id: None,
            wca_id: None,
            attempt_count,
            time_limit_info: None,
            is_blank: false,
            is_cover_sheet: true,
            total_group_cards,
        }
    }

    /// Creates a blank scorecard item for a subsequent round.
    #[allow(clippy::too_many_arguments)]
    pub fn blank(
        competition_name: &'a str,
        event: WcaEvent,
        round_number: usize,
        group_number: usize,
        stage_name: Option<&'a str>,
        attempt_count: usize,
        time_limit_info: Option<TimeLimitInfo>,
    ) -> Self {
        Self {
            scorecard_number: 0,
            station_number: None,
            competition_name,
            event,
            round_number,
            group_number,
            stage_name,
            competitor_name: "",
            registrant_id: None,
            wca_id: None,
            attempt_count,
            time_limit_info,
            is_blank: true,
            is_cover_sheet: false,
            total_group_cards: 0,
        }
    }

    /// Returns formatted competitor name.
    pub fn display_competitor_name(&self) -> &str {
        if self.is_blank {
            ""
        } else {
            self.competitor_name
        }
    }

    /// Returns WCA ID string or empty string if none.
    pub fn display_wca_id(&self) -> &str {
        if self.is_blank {
            ""
        } else {
            self.wca_id.unwrap_or("")
        }
    }

    /// Returns truncated competition name if exceeding `max_chars`.
    pub fn truncated_competition_name(&self, max_chars: usize) -> Cow<'_, str> {
        if self.competition_name.chars().count() > max_chars {
            let truncated: String = self
                .competition_name
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect();
            Cow::Owned(format!("{truncated}..."))
        } else {
            Cow::Borrowed(self.competition_name)
        }
    }

    /// Returns formatted time limit and cutoff info if present.
    pub fn formatted_time_limit_info(&self) -> Option<String> {
        self.time_limit_info.map(|info| info.format_display())
    }
}

/// Summary information for a planned competition round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedRoundSummary {
    OpenRound {
        event: WcaEvent,
        round_number: usize,
        competitor_count: usize,
        sample_competitor_names: Vec<String>,
    },
    SubsequentRound {
        event: WcaEvent,
        round_number: usize,
        blank_count: usize,
        reason: String,
    },
}

impl PlannedRoundSummary {
    /// Formats the round summary into human-readable text for console output.
    pub fn format(&self) -> String {
        match self {
            Self::OpenRound {
                event,
                round_number,
                competitor_count,
                sample_competitor_names,
            } => {
                let round_type = if *round_number == 1 {
                    "(Open Round)"
                } else {
                    "(Assigned Competitors)"
                };
                let event_id = event.code();
                let mut s = format!(
                    "[{event_id} Round {round_number}] {round_type} -> Generating scorecards for {competitor_count} accepted competitors\n"
                );
                if *competitor_count <= 5 && *competitor_count > 0 {
                    let _ = writeln!(s, "   Competitors: {}", sample_competitor_names.join(", "));
                }
                s
            }
            Self::SubsequentRound {
                event,
                round_number,
                blank_count,
                reason,
            } => {
                let event_id = event.code();
                format!(
                    "[{event_id} Round {round_number}] (Subsequent Round) -> Generating {blank_count} blank scorecards ({reason})\n"
                )
            }
        }
    }
}

/// `ScorecardPlan` contains all generated scorecard items along with round summaries and diagnostic notes.
#[derive(Debug, Clone, Default)]
pub struct ScorecardPlan<'a> {
    pub items: Vec<ScorecardItem<'a>>,
    pub summaries: Vec<PlannedRoundSummary>,
    pub notes: Vec<String>,
}

impl ScorecardPlan<'_> {
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
        let mut num = 1;
        for card in &mut self.items {
            if card.is_cover_sheet {
                card.scorecard_number = 0;
            } else {
                card.scorecard_number = num;
                num += 1;
            }
        }
    }

    /// Formats the plan summary as a readable table with Event, Round, Status, and Competitors columns.
    pub fn format_summary(&self) -> String {
        let mut lines = Vec::new();
        let mut row_buf = String::new();

        for note in &self.notes {
            lines.push(format!("Note: {note}"));
        }

        if !self.summaries.is_empty() {
            let sep_char = '─';
            lines.push(format!(
                "{:<10} {:<10} {:<12} {}",
                "Event", "Round", "Status", "Competitors"
            ));
            lines.push(format!(
                "{:<10} {:<10} {:<12} {}",
                sep_char.to_string().repeat(8),
                sep_char.to_string().repeat(8),
                sep_char.to_string().repeat(10),
                sep_char.to_string().repeat(14),
            ));

            for summary in &self.summaries {
                row_buf.clear();
                match summary {
                    PlannedRoundSummary::OpenRound {
                        event,
                        round_number,
                        competitor_count,
                        ..
                    } => {
                        let status = if *round_number == 1 {
                            "Open"
                        } else {
                            "Assigned"
                        };
                        let event_id = event.code();
                        let _ = write!(
                            row_buf,
                            "{event_id:<10} {round_number:<10} {status:<12} {competitor_count}"
                        );
                    }
                    PlannedRoundSummary::SubsequentRound {
                        event,
                        round_number,
                        blank_count,
                        reason,
                    } => {
                        let status = "Subsequent";
                        let event_id = event.code();
                        let _ = write!(
                            row_buf,
                            "{event_id:<10} {round_number:<10} {status:<12} {blank_count} blank ({reason})"
                        );
                    }
                }
                lines.push(row_buf.clone());
            }
        }

        crate::progress::draw_box("Scorecards Generated", &lines)
    }
}

impl std::fmt::Display for ScorecardPlan<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_summary())
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
