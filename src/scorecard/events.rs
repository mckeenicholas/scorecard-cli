use std::fmt;
use std::str::FromStr;

/// Official World Cube Association (WCA) events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WcaEvent {
    E333,
    E222,
    E444,
    E555,
    E666,
    E777,
    E333Bf,
    E333Oh,
    E333Fm,
    Minx,
    Pyram,
    Clock,
    Skewb,
    Sq1,
    E444Bf,
    E555Bf,
    E333Mbf,
}

/// Error returned when parsing an invalid or unknown WCA event ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseEventError(pub String);

impl fmt::Display for ParseEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown or unsupported WCA event ID: '{}'", self.0)
    }
}

impl std::error::Error for ParseEventError {}

impl WcaEvent {
    /// All 17 official WCA events in canonical order.
    pub const ALL: [Self; 17] = [
        Self::E333,
        Self::E222,
        Self::E444,
        Self::E555,
        Self::E666,
        Self::E777,
        Self::E333Bf,
        Self::E333Oh,
        Self::E333Fm,
        Self::Minx,
        Self::Pyram,
        Self::Clock,
        Self::Skewb,
        Self::Sq1,
        Self::E444Bf,
        Self::E555Bf,
        Self::E333Mbf,
    ];

    /// Returns the standard WCA event ID string (e.g. `"333"`, `"minx"`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::E333 => "333",
            Self::E222 => "222",
            Self::E444 => "444",
            Self::E555 => "555",
            Self::E666 => "666",
            Self::E777 => "777",
            Self::E333Bf => "333bf",
            Self::E333Oh => "333oh",
            Self::E333Fm => "333fm",
            Self::Minx => "minx",
            Self::Pyram => "pyram",
            Self::Clock => "clock",
            Self::Skewb => "skewb",
            Self::Sq1 => "sq1",
            Self::E444Bf => "444bf",
            Self::E555Bf => "555bf",
            Self::E333Mbf => "333mbf",
        }
    }

    /// Returns the user-friendly display name (e.g. `"3x3x3 Cube"`).
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::E333 => "3x3x3 Cube",
            Self::E222 => "2x2x2 Cube",
            Self::E444 => "4x4x4 Cube",
            Self::E555 => "5x5x5 Cube",
            Self::E666 => "6x6x6 Cube",
            Self::E777 => "7x7x7 Cube",
            Self::E333Bf => "3x3x3 Blindfolded",
            Self::E333Oh => "3x3x3 One-Handed",
            Self::E333Fm => "3x3x3 Fewest Moves",
            Self::Minx => "Megaminx",
            Self::Pyram => "Pyraminx",
            Self::Clock => "Rubik's Clock",
            Self::Skewb => "Skewb",
            Self::Sq1 => "Square-1",
            Self::E444Bf => "4x4x4 Blindfolded",
            Self::E555Bf => "5x5x5 Blindfolded",
            Self::E333Mbf => "3x3x3 Multi-Blind",
        }
    }

    /// Parses a standard WCA event ID string into a `WcaEvent`.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "333" => Some(Self::E333),
            "222" => Some(Self::E222),
            "444" => Some(Self::E444),
            "555" => Some(Self::E555),
            "666" => Some(Self::E666),
            "777" => Some(Self::E777),
            "333bf" => Some(Self::E333Bf),
            "333oh" => Some(Self::E333Oh),
            "333fm" => Some(Self::E333Fm),
            "minx" => Some(Self::Minx),
            "pyram" => Some(Self::Pyram),
            "clock" => Some(Self::Clock),
            "skewb" => Some(Self::Skewb),
            "sq1" => Some(Self::Sq1),
            "444bf" => Some(Self::E444Bf),
            "555bf" => Some(Self::E555Bf),
            "333mbf" => Some(Self::E333Mbf),
            _ => None,
        }
    }
}

impl AsRef<str> for WcaEvent {
    fn as_ref(&self) -> &str {
        self.code()
    }
}

impl fmt::Display for WcaEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl TryFrom<&str> for WcaEvent {
    type Error = ParseEventError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_id(value).ok_or_else(|| ParseEventError(value.to_string()))
    }
}

impl FromStr for WcaEvent {
    type Err = ParseEventError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

/// Maps standard WCA event IDs to their user-friendly display names, or returns `None` if unrecognized.
#[must_use]
pub fn event_name_by_id(id: &str) -> Option<&'static str> {
    WcaEvent::from_id(id).map(WcaEvent::display_name)
}

/// Strongly typed representation of an official WCA round or group activity code (e.g. `"333-r1"`, `"333-r2-g1"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActivityCode {
    pub event: WcaEvent,
    pub round_number: usize,
    pub group_number: Option<usize>,
}

/// Error returned when parsing an invalid activity code string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseActivityCodeError(pub String);

impl fmt::Display for ParseActivityCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid WCA activity code: '{}'", self.0)
    }
}

impl std::error::Error for ParseActivityCodeError {}

impl ActivityCode {
    /// Creates an `ActivityCode` for an entire round (no specific group).
    #[must_use]
    pub const fn round(event: WcaEvent, round_number: usize) -> Self {
        Self {
            event,
            round_number,
            group_number: None,
        }
    }

    /// Creates an `ActivityCode` for a specific round and group.
    #[cfg(test)]
    #[must_use]
    pub const fn group(event: WcaEvent, round_number: usize, group_number: usize) -> Self {
        Self {
            event,
            round_number,
            group_number: Some(group_number),
        }
    }

    /// Parses an activity code string (e.g. `"333-r1"`, `"333-r2-g1"`).
    /// Returns `None` if the string does not conform to official WCA activity code syntax or references an unrecognized event.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts = s.split('-');
        let event_str = parts.next()?;
        let round_str = parts.next()?;
        let group_str = parts.next();
        if parts.next().is_some() {
            return None;
        }

        let event = WcaEvent::from_id(event_str)?;
        let round_num_str = round_str.strip_prefix('r')?;
        let round_number = round_num_str.parse::<usize>().ok()?;
        if round_number == 0 {
            return None;
        }

        let group_number = match group_str {
            Some(g) => {
                let g_num_str = g.strip_prefix('g')?;
                let g_num = g_num_str.parse::<usize>().ok()?;
                if g_num == 0 {
                    return None;
                }
                Some(g_num)
            }
            None => None,
        };

        Some(Self {
            event,
            round_number,
            group_number,
        })
    }

    /// Returns `true` if this activity code belongs to the specified event and round.
    #[must_use]
    pub fn matches_round(&self, event: WcaEvent, round_number: usize) -> bool {
        self.event == event && self.round_number == round_number
    }

    /// Returns the group number, defaulting to 1 if not specified.
    #[must_use]
    pub fn group_or_default(&self) -> usize {
        self.group_number.unwrap_or(1)
    }

    /// Formats the round identifier (e.g. `"333-r1"`).
    #[must_use]
    pub fn round_id(&self) -> String {
        format!("{}-r{}", self.event.code(), self.round_number)
    }
}

impl fmt::Display for ActivityCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-r{}", self.event.code(), self.round_number)?;
        if let Some(group) = self.group_number {
            write!(f, "-g{group}")?;
        }
        Ok(())
    }
}

impl TryFrom<&str> for ActivityCode {
    type Error = ParseActivityCodeError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value).ok_or_else(|| ParseActivityCodeError(value.to_string()))
    }
}

impl FromStr for ActivityCode {
    type Err = ParseActivityCodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}
