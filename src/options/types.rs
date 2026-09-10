use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Criteria for splitting output PDFs into separate files.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, ValueEnum, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum SplitBy {
    #[value(name = "event", alias = "events", alias = "e")]
    Event,
    #[value(name = "group", alias = "groups", alias = "g")]
    Group,
    #[value(
        name = "stage",
        alias = "stages",
        alias = "room",
        alias = "rooms",
        alias = "s"
    )]
    Stage,
}

impl Display for SplitBy {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SplitBy::Event => write!(f, "event"),
            SplitBy::Group => write!(f, "group"),
            SplitBy::Stage => write!(f, "stage"),
        }
    }
}

/// Error returned when parsing an invalid split criterion string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseSplitByError(pub String);

impl Display for ParseSplitByError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid split criterion '{}': must be 'event', 'group', or 'stage'",
            self.0
        )
    }
}

impl Error for ParseSplitByError {}

impl FromStr for SplitBy {
    type Err = ParseSplitByError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as ValueEnum>::from_str(s, true).map_err(|_| ParseSplitByError(s.to_string()))
    }
}

/// Criteria for adding cover sheets.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, ValueEnum, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum CoverSheetBy {
    #[value(
        name = "round",
        alias = "rounds",
        alias = "r",
        alias = "event",
        alias = "events",
        alias = "e"
    )]
    Round,
    #[value(name = "group", alias = "groups", alias = "g")]
    Group,
    #[value(
        name = "stage",
        alias = "stages",
        alias = "room",
        alias = "rooms",
        alias = "s"
    )]
    Stage,
    #[value(name = "none", alias = "false", alias = "off")]
    None,
}

impl CoverSheetBy {
    /// Returns the hierarchy tier of the cover sheet criterion (1 is highest tier).
    pub fn tier(self) -> u8 {
        match self {
            CoverSheetBy::Round => 1,
            CoverSheetBy::Group => 2,
            CoverSheetBy::Stage => 3,
            CoverSheetBy::None => 255,
        }
    }
}

impl Display for CoverSheetBy {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CoverSheetBy::Round => write!(f, "round"),
            CoverSheetBy::Group => write!(f, "group"),
            CoverSheetBy::Stage => write!(f, "stage"),
            CoverSheetBy::None => write!(f, "none"),
        }
    }
}

/// Error returned when parsing an invalid cover sheet criterion string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseCoverSheetByError(pub String);

impl Display for ParseCoverSheetByError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid cover sheet criterion '{}': must be 'round' ('r'), 'group' ('g'), or 'stage' ('s')",
            self.0
        )
    }
}

impl Error for ParseCoverSheetByError {}

impl FromStr for CoverSheetBy {
    type Err = ParseCoverSheetByError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as ValueEnum>::from_str(s, true).map_err(|_| ParseCoverSheetByError(s.to_string()))
    }
}

/// Error returned when options have incompatible file splitting and cover sheet configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsCompatibilityError {
    /// PDF file splitting is more specific than cover sheet criteria.
    IncompatibleSplit {
        split: SplitBy,
        cover_sheet: CoverSheetBy,
    },
}

impl Display for OptionsCompatibilityError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            OptionsCompatibilityError::IncompatibleSplit { split, cover_sheet } => {
                match (cover_sheet, split) {
                    (CoverSheetBy::Round, SplitBy::Stage) => write!(
                        f,
                        "Incompatible configuration: Splitting PDF files by 'stage' is more specific than 'round' cover sheet. A round cover sheet cannot be placed in a single stage PDF file."
                    ),
                    (CoverSheetBy::Round, SplitBy::Group) => write!(
                        f,
                        "Incompatible configuration: Splitting PDF files by 'group' is more specific than 'round' cover sheet. A round cover sheet cannot be placed in a single group PDF file."
                    ),
                    (CoverSheetBy::Group, SplitBy::Stage) => write!(
                        f,
                        "Incompatible configuration: Splitting PDF files by 'stage' is more specific than 'group' cover sheet. A group cover sheet covers the entire group across stages."
                    ),
                    _ => write!(
                        f,
                        "Incompatible configuration: Splitting PDF files by '{split}' is more specific than '{cover_sheet}' cover sheet."
                    ),
                }
            }
        }
    }
}

impl Error for OptionsCompatibilityError {}
