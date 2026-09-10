use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, Error as IoError};

use crossterm::{cursor, execute, terminal};
use inquire::InquireError;
use serde::Deserialize;

use crate::scorecard::RoundId;
use crate::wcif::WcifLoadError;

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub value: String,
    pub display: String,
}

#[derive(Debug, Clone)]
pub struct RoundChoice {
    pub round_id: RoundId,
    pub display: String,
}

impl Display for RoundChoice {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverSheetChoice {
    Round,
    Group,
    Stage,
}

impl Display for CoverSheetChoice {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CoverSheetChoice::Round => write!(f, "By Round (one for the entire round)"),
            CoverSheetChoice::Group => write!(f, "By Group (one per group across all stages)"),
            CoverSheetChoice::Stage => write!(f, "By Stage (one per group on each stage)"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtraOption {
    StartGroupOnNewPage,
    PrintStations,
    LocalNamesFirst,
    PrintOneName,
    ScrambleCheckerTopRanked,
    ScrambleCheckerFinalRounds,
    ScrambleCheckerBlank,
}

impl Display for ExtraOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ExtraOption::StartGroupOnNewPage => {
                write!(
                    f,
                    "Start group on new page (insert blank spaces to align top-left)"
                )
            }
            ExtraOption::PrintStations => write!(f, "Print station numbers"),
            ExtraOption::LocalNamesFirst => write!(f, "Display local names first"),
            ExtraOption::PrintOneName => write!(f, "Only print one name"),
            ExtraOption::ScrambleCheckerTopRanked => {
                write!(f, "Scramble checker for top ranked competitors")
            }
            ExtraOption::ScrambleCheckerFinalRounds => {
                write!(f, "Scramble checker for final rounds")
            }
            ExtraOption::ScrambleCheckerBlank => {
                write!(f, "Scramble checker for blank scorecards")
            }
        }
    }
}

pub struct ExtraFlags {
    pub start_group_on_new_page: bool,
    pub print_stations: bool,
    pub local_names_first: bool,
    pub print_one_name: bool,
    pub scramble_checker_top_ranked: bool,
    pub scramble_checker_final_rounds: bool,
    pub scramble_checker_blank: bool,
}

/// Error encountered during the interactive terminal prompt flow.
#[derive(Debug)]
pub enum InteractiveError {
    Io(IoError),
    Prompt(InquireError),
    Wcif(WcifLoadError),
    Aborted,
}

impl Display for InteractiveError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            InteractiveError::Io(e) => write!(f, "Interactive I/O error::Error: {e}"),
            InteractiveError::Prompt(e) => write!(f, "Interactive prompt error::Error: {e}"),
            InteractiveError::Wcif(e) => write!(f, "{e}"),
            InteractiveError::Aborted => write!(f, "Interactive flow aborted by user"),
        }
    }
}

impl Error for InteractiveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            InteractiveError::Io(e) => Some(e),
            InteractiveError::Prompt(e) => Some(e),
            InteractiveError::Wcif(e) => Some(e),
            InteractiveError::Aborted => None,
        }
    }
}

impl From<IoError> for InteractiveError {
    fn from(err: IoError) -> Self {
        InteractiveError::Io(err)
    }
}

impl From<InquireError> for InteractiveError {
    fn from(err: InquireError) -> Self {
        InteractiveError::Prompt(err)
    }
}

impl From<WcifLoadError> for InteractiveError {
    fn from(err: WcifLoadError) -> Self {
        InteractiveError::Wcif(err)
    }
}

pub struct RawModeGuard;

impl RawModeGuard {
    pub fn enter() -> Result<Self, IoError> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), cursor::Show);
    }
}

#[derive(Deserialize)]
pub struct WcaItem {
    pub id: String,
    pub name: String,
    pub country_iso2: Option<String>,
}
