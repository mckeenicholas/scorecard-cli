use std::borrow::Cow;
use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{self, Display, Formatter, Write as _};
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::slice::Iter;
use std::vec::IntoIter;

use super::events::WcaEvent;
use super::planner;
use crate::progress;
use crate::wcif::{Cutoff, Person, TimeLimit, WcaId};

pub type RoundNumber = u32;
pub type GroupNumber = u32;

/// Error returned when attempting to construct a [`WcaResult`] with an invalid value (< -2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidWcaResult(pub i32);

impl Display for InvalidWcaResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid WCA result: {} centiseconds (values < -2 are not allowed)",
            self.0
        )
    }
}

impl Error for InvalidWcaResult {}

/// Represents a WCA attempt result or time (in centiseconds >= -2).
///
/// Invariants:
/// - `-2`: DNS (Did Not Start)
/// - `-1`: DNF (Did Not Finish)
/// - `0`: Skipped / No attempt
/// - `> 0`: Valid solve time in centiseconds
///
/// Values `< -2` are disallowed and rejected by constructors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct WcaResult(i32);

impl WcaResult {
    const DNF_RESULT: i32 = -1;
    const DNS_RESULT: i32 = -2;

    pub const DNF: Self = Self(Self::DNF_RESULT);
    pub const DNS: Self = Self(Self::DNS_RESULT);

    /// Constructs a `WcaResult` if `centiseconds >= -2`.
    /// Returns `None` if `centiseconds < -2`.
    #[inline]
    pub const fn new(centiseconds: i32) -> Option<Self> {
        if centiseconds < -2 {
            None
        } else {
            Some(Self(centiseconds))
        }
    }

    /// Constructs a `WcaResult` if `centiseconds >= -2`.
    /// Returns `Err(InvalidWcaResult)` if `centiseconds < -2`.
    #[inline]
    pub const fn try_new(centiseconds: i32) -> Result<Self, InvalidWcaResult> {
        if centiseconds < -2 {
            Err(InvalidWcaResult(centiseconds))
        } else {
            Ok(Self(centiseconds))
        }
    }

    #[inline]
    pub const fn centiseconds(self) -> i32 {
        self.0
    }

    #[inline]
    pub const fn is_valid_time(self) -> bool {
        self.0 > 0
    }

    #[inline]
    pub const fn is_dnf(self) -> bool {
        self.0 == Self::DNF_RESULT
    }

    #[inline]
    pub const fn is_dns(self) -> bool {
        self.0 == Self::DNS_RESULT
    }

    /// Constructs a `WcaResult` only for strictly positive times (centiseconds > 0).
    /// Returns `None` for sentinels (<= 0) or invalid values.
    #[inline]
    pub const fn from_centiseconds(centis: i32) -> Option<Self> {
        if centis > 0 { Some(Self(centis)) } else { None }
    }
}

impl TryFrom<i32> for WcaResult {
    type Error = InvalidWcaResult;

    #[inline]
    fn try_from(centis: i32) -> Result<Self, Self::Error> {
        Self::try_new(centis)
    }
}

impl Deref for WcaResult {
    type Target = i32;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq<i32> for WcaResult {
    #[inline]
    fn eq(&self, other: &i32) -> bool {
        self.0 == *other
    }
}

impl PartialEq<WcaResult> for i32 {
    #[inline]
    fn eq(&self, other: &WcaResult) -> bool {
        *self == other.0
    }
}

impl PartialOrd<i32> for WcaResult {
    #[inline]
    fn partial_cmp(&self, other: &i32) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl PartialOrd<WcaResult> for i32 {
    #[inline]
    fn partial_cmp(&self, other: &WcaResult) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl Display for WcaResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.0 <= 0 {
            return match self.0 {
                -1 => write!(f, "DNF"),
                -2 => write!(f, "DNS"),
                _ => write!(f, "None"),
            };
        }

        let total_seconds = self.0 / 100;
        let cs = self.0 % 100;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;

        if minutes > 0 {
            if cs > 0 {
                write!(f, "{minutes}:{seconds:02}.{cs:02}")
            } else {
                write!(f, "{minutes}:{seconds:02}.00")
            }
        } else {
            write!(f, "{seconds}.{cs:02}")
        }
    }
}

/// Compact, Copy-able metadata about a round's time limit and cutoff.
/// Captures WCA results and attempt counts without any heap allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimeLimitInfo {
    pub limit_centiseconds: Option<WcaResult>,
    pub is_cumulative: bool,
    pub cutoff_centiseconds: Option<WcaResult>,
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
            limit_centiseconds: time_limit.and_then(|tl| WcaResult::new(tl.centiseconds)),
            is_cumulative: time_limit
                .and_then(|tl| tl.cumulative_round_ids.as_ref())
                .is_some_and(|ids| !ids.is_empty()),
            cutoff_centiseconds: cutoff.and_then(|c| WcaResult::new(c.attempt_result)),
            cutoff_attempts: cutoff.map_or(0, |c| c.number_of_attempts),
        })
    }

    /// Formats centiseconds into a human-readable time string (e.g. "1:30.50").
    /// Returns `None` for values <= 0 (covers WCA sentinels: -1 = DNF, -2 = DNS).
    pub fn format_centiseconds(centis: i32) -> Option<String> {
        if centis <= 0 {
            None
        } else {
            WcaResult::new(centis).map(|r| r.to_string())
        }
    }

    /// Formats the cutoff and time limit info into a display string for scorecard footers.
    pub fn format_display(&self) -> String {
        let mut s = String::with_capacity(48);
        if let Some(cutoff) = self.cutoff_centiseconds {
            let _ = write!(s, "Cutoff: < {cutoff} ({} att)", self.cutoff_attempts);
        }

        if let Some(limit) = self.limit_centiseconds {
            if !s.is_empty() {
                s.push_str("  |  ");
            }
            if self.is_cumulative {
                let _ = write!(s, "Time limit: {limit} cumulative");
            } else {
                let _ = write!(s, "Time limit: {limit}");
            }
        }

        s
    }
}

/// Represents a competitor's identity on a scorecard without heap allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Competitor<'a> {
    pub name: &'a str,
    pub local_name: Option<&'a str>,
    pub registrant_id: NonZeroUsize,
    pub wca_id: Option<WcaId>,
}

impl<'a> Competitor<'a> {
    /// Creates a new `Competitor`.
    pub const fn new(
        name: &'a str,
        local_name: Option<&'a str>,
        registrant_id: NonZeroUsize,
        wca_id: Option<WcaId>,
    ) -> Self {
        Self {
            name,
            local_name,
            registrant_id,
            wca_id,
        }
    }

    /// Convenience constructor for tests with only a name (defaults `registrant_id` to 1).
    pub const fn simple(name: &'a str) -> Self {
        Self {
            name,
            local_name: None,
            registrant_id: NonZeroUsize::MIN,
            wca_id: None,
        }
    }

    /// Constructs a `Competitor` from a WCIF `Person`.
    pub fn from_person(person: &'a Person, print_one_name: bool, local_names_first: bool) -> Self {
        let (name, local_name) =
            planner::format_competitor_name(&person.name, print_one_name, local_names_first);
        Self {
            name,
            local_name,
            registrant_id: person.registrant_id().unwrap_or(NonZeroUsize::MIN),
            wca_id: person.wca_id,
        }
    }

    /// Returns the `(primary_name, Option<local_name>)` pair.
    pub fn display_name(&self) -> (&'a str, Option<&'a str>) {
        (self.name, self.local_name)
    }

    /// Returns the WCA ID string or empty string.
    pub fn display_wca_id(&self) -> &str {
        self.wca_id.as_ref().map_or("", |w| w.as_str())
    }
}

/// Helper to truncate competition name to `max_chars` with an ellipsis if necessary.
pub fn truncate_comp_name(name: &str, max_chars: usize) -> Cow<'_, str> {
    if name.chars().count() > max_chars {
        let mut s = String::with_capacity(max_chars);
        s.extend(name.chars().take(max_chars.saturating_sub(3)));
        s.push_str("...");
        Cow::Owned(s)
    } else {
        Cow::Borrowed(name)
    }
}

/// Scorecard containing all details for an assigned competitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scorecard<'a> {
    pub number: usize,
    pub station_number: Option<usize>,
    pub competition_name: &'a str,
    pub event: WcaEvent,
    pub round_number: RoundNumber,
    pub group_number: GroupNumber,
    pub stage_name: Option<&'a str>,
    pub competitor: Competitor<'a>,
    pub attempt_count: usize,
    pub time_limit_info: Option<TimeLimitInfo>,
    pub needs_scramble_checker: bool,
}

impl<'a> Scorecard<'a> {
    /// Creates a new `Scorecard` with default settings (5 attempts, unnumbered, no station/stage/limit/checker).
    pub const fn new(
        competition_name: &'a str,
        event: WcaEvent,
        round_number: RoundNumber,
        group_number: GroupNumber,
        competitor: Competitor<'a>,
    ) -> Self {
        Self {
            number: 0,
            station_number: None,
            competition_name,
            event,
            round_number,
            group_number,
            stage_name: None,
            competitor,
            attempt_count: 5,
            time_limit_info: None,
            needs_scramble_checker: false,
        }
    }

    #[must_use]
    pub const fn with_number(mut self, number: usize) -> Self {
        self.number = number;
        self
    }

    #[must_use]
    pub const fn with_station(mut self, station_number: Option<usize>) -> Self {
        self.station_number = station_number;
        self
    }

    #[must_use]
    pub const fn with_stage(mut self, stage_name: Option<&'a str>) -> Self {
        self.stage_name = stage_name;
        self
    }

    #[must_use]
    pub const fn with_attempts(mut self, attempt_count: usize) -> Self {
        self.attempt_count = attempt_count;
        self
    }

    #[must_use]
    pub const fn with_time_limit(mut self, time_limit_info: Option<TimeLimitInfo>) -> Self {
        self.time_limit_info = time_limit_info;
        self
    }

    #[must_use]
    pub const fn with_scramble_checker(mut self, needs_scramble_checker: bool) -> Self {
        self.needs_scramble_checker = needs_scramble_checker;
        self
    }

    #[must_use]
    pub const fn event_id(&self) -> &'static str {
        self.event.code()
    }

    #[must_use]
    pub const fn event_name(&self) -> &'static str {
        self.event.display_name()
    }

    pub fn truncated_competition_name(&self, max_chars: usize) -> Cow<'_, str> {
        truncate_comp_name(self.competition_name, max_chars)
    }

    pub fn formatted_time_limit_info(&self) -> Option<String> {
        self.time_limit_info.map(|info| info.format_display())
    }
}

/// Blank scorecard printed for subsequent rounds or manual scorekeeping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlankScorecard<'a> {
    pub number: usize,
    pub station_number: Option<usize>,
    pub competition_name: &'a str,
    pub event: WcaEvent,
    pub round_number: RoundNumber,
    pub group_number: GroupNumber,
    pub stage_name: Option<&'a str>,
    pub attempt_count: usize,
    pub time_limit_info: Option<TimeLimitInfo>,
    pub needs_scramble_checker: bool,
}

impl<'a> BlankScorecard<'a> {
    /// Creates a new `BlankScorecard` with default settings (5 attempts, unnumbered, no station/stage/limit/checker).
    pub const fn new(
        competition_name: &'a str,
        event: WcaEvent,
        round_number: RoundNumber,
        group_number: GroupNumber,
    ) -> Self {
        Self {
            number: 0,
            station_number: None,
            competition_name,
            event,
            round_number,
            group_number,
            stage_name: None,
            attempt_count: 5,
            time_limit_info: None,
            needs_scramble_checker: false,
        }
    }

    #[must_use]
    pub const fn with_number(mut self, number: usize) -> Self {
        self.number = number;
        self
    }

    #[must_use]
    pub const fn with_station(mut self, station_number: Option<usize>) -> Self {
        self.station_number = station_number;
        self
    }

    #[must_use]
    pub const fn with_stage(mut self, stage_name: Option<&'a str>) -> Self {
        self.stage_name = stage_name;
        self
    }

    #[must_use]
    pub const fn with_attempts(mut self, attempt_count: usize) -> Self {
        self.attempt_count = attempt_count;
        self
    }

    #[must_use]
    pub const fn with_time_limit(mut self, time_limit_info: Option<TimeLimitInfo>) -> Self {
        self.time_limit_info = time_limit_info;
        self
    }

    #[must_use]
    pub const fn with_scramble_checker(mut self, needs_scramble_checker: bool) -> Self {
        self.needs_scramble_checker = needs_scramble_checker;
        self
    }

    #[must_use]
    pub const fn event_id(&self) -> &'static str {
        self.event.code()
    }

    #[must_use]
    pub const fn event_name(&self) -> &'static str {
        self.event.display_name()
    }

    pub fn truncated_competition_name(&self, max_chars: usize) -> Cow<'_, str> {
        truncate_comp_name(self.competition_name, max_chars)
    }

    pub fn formatted_time_limit_info(&self) -> Option<String> {
        self.time_limit_info.map(|info| info.format_display())
    }
}

/// Cover sheet preceding scorecards for a round, group, or stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverSheet<'a> {
    pub competition_name: &'a str,
    pub event: WcaEvent,
    pub round_number: RoundNumber,
    pub group_number: GroupNumber,
    pub stage_name: Option<&'a str>,
    pub total_group_cards: usize,
}

impl<'a> CoverSheet<'a> {
    /// Creates a new `CoverSheet`.
    pub const fn new(
        competition_name: &'a str,
        event: WcaEvent,
        round_number: RoundNumber,
        group_number: GroupNumber,
        total_group_cards: usize,
    ) -> Self {
        Self {
            competition_name,
            event,
            round_number,
            group_number,
            stage_name: None,
            total_group_cards,
        }
    }

    #[must_use]
    pub const fn with_stage(mut self, stage_name: Option<&'a str>) -> Self {
        self.stage_name = stage_name;
        self
    }

    #[must_use]
    pub const fn event_id(&self) -> &'static str {
        self.event.code()
    }

    #[must_use]
    pub const fn event_name(&self) -> &'static str {
        self.event.display_name()
    }

    pub fn truncated_competition_name(&self, max_chars: usize) -> Cow<'_, str> {
        truncate_comp_name(self.competition_name, max_chars)
    }
}

/// `ScorecardItem` contains either a competitor scorecard, blank scorecard, cover sheet, or an empty padding space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScorecardItem<'a> {
    Scorecard(Scorecard<'a>),
    Blank(BlankScorecard<'a>),
    CoverSheet(CoverSheet<'a>),
    #[default]
    Empty,
}

impl<'a> From<Scorecard<'a>> for ScorecardItem<'a> {
    #[inline]
    fn from(sc: Scorecard<'a>) -> Self {
        Self::Scorecard(sc)
    }
}

impl<'a> From<BlankScorecard<'a>> for ScorecardItem<'a> {
    #[inline]
    fn from(blank: BlankScorecard<'a>) -> Self {
        Self::Blank(blank)
    }
}

impl<'a> From<CoverSheet<'a>> for ScorecardItem<'a> {
    #[inline]
    fn from(cover: CoverSheet<'a>) -> Self {
        Self::CoverSheet(cover)
    }
}

#[cfg(test)]
impl Default for Scorecard<'static> {
    fn default() -> Self {
        Self {
            number: 1,
            station_number: Some(1),
            competition_name: "Test Comp",
            event: WcaEvent::E333,
            round_number: 1,
            group_number: 1,
            stage_name: Some("Main Stage"),
            competitor: Competitor::simple("Alice"),
            attempt_count: 5,
            time_limit_info: None,
            needs_scramble_checker: false,
        }
    }
}

impl<'a> ScorecardItem<'a> {
    #[must_use]
    pub const fn is_cover_sheet(&self) -> bool {
        matches!(self, Self::CoverSheet(_))
    }

    #[must_use]
    pub const fn is_blank(&self) -> bool {
        matches!(self, Self::Blank(_))
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Creates a competitor scorecard item from a `Scorecard`.
    #[must_use]
    pub const fn scorecard(card: Scorecard<'a>) -> Self {
        Self::Scorecard(card)
    }

    /// Creates a cover sheet item from a `CoverSheet`.
    #[must_use]
    pub const fn cover_sheet(sheet: CoverSheet<'a>) -> Self {
        Self::CoverSheet(sheet)
    }

    /// Creates a blank scorecard item from a `BlankScorecard`.
    #[must_use]
    pub const fn blank(card: BlankScorecard<'a>) -> Self {
        Self::Blank(card)
    }

    /// Creates an empty space item used to pad page grids so that groups start on a new page.
    #[must_use]
    pub const fn empty_space() -> Self {
        Self::Empty
    }
}

/// Summary information for a planned competition round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedRoundSummary {
    OpenRound {
        event: WcaEvent,
        round_number: RoundNumber,
        competitor_count: usize,
        sample_competitor_names: Vec<String>,
    },
    SubsequentRound {
        event: WcaEvent,
        round_number: RoundNumber,
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
                    let _ = write!(s, "   Competitors: ");
                    for (i, name) in sample_competitor_names.iter().enumerate() {
                        if i > 0 {
                            s.push_str(", ");
                        }
                        s.push_str(name);
                    }
                    s.push('\n');
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

    pub fn assign_numbers(&mut self) {
        let mut num = 1;
        for card in &mut self.items {
            match card {
                ScorecardItem::Scorecard(sc) => {
                    sc.number = num;
                    num += 1;
                }
                ScorecardItem::Blank(b) => {
                    b.number = num;
                    num += 1;
                }
                ScorecardItem::CoverSheet(_) | ScorecardItem::Empty => {}
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
            lines.push("Event      Round      Status       Competitors".to_owned());
            lines.push("────────   ────────   ──────────   ──────────────".to_owned());

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

        progress::draw_box("Scorecards Generated", &lines)
    }
}

impl Display for ScorecardPlan<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_summary())
    }
}

impl<'a> Deref for ScorecardPlan<'a> {
    type Target = [ScorecardItem<'a>];

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

impl<'a> IntoIterator for ScorecardPlan<'a> {
    type Item = ScorecardItem<'a>;
    type IntoIter = IntoIter<ScorecardItem<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, 'b> IntoIterator for &'b ScorecardPlan<'a> {
    type Item = &'b ScorecardItem<'a>;
    type IntoIter = Iter<'b, ScorecardItem<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}
