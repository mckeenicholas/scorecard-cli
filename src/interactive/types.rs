use crossterm::{cursor, execute, terminal};
use std::io::stdout;

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub value: String,
    pub display: String,
}

#[derive(Debug, Clone)]
pub struct RoundChoice {
    pub round_id: String,
    pub display: String,
}

impl std::fmt::Display for RoundChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverSheetChoice {
    Round,
    Group,
    Stage,
}

impl std::fmt::Display for CoverSheetChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl std::fmt::Display for ExtraOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
    Io(std::io::Error),
    Prompt(inquire::InquireError),
    Wcif(crate::wcif::WcifLoadError),
    Aborted,
}

impl std::fmt::Display for InteractiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InteractiveError::Io(e) => write!(f, "Interactive I/O error: {e}"),
            InteractiveError::Prompt(e) => write!(f, "Interactive prompt error: {e}"),
            InteractiveError::Wcif(e) => write!(f, "{e}"),
            InteractiveError::Aborted => write!(f, "Interactive flow aborted by user"),
        }
    }
}

impl std::error::Error for InteractiveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            InteractiveError::Io(e) => Some(e),
            InteractiveError::Prompt(e) => Some(e),
            InteractiveError::Wcif(e) => Some(e),
            InteractiveError::Aborted => None,
        }
    }
}

impl From<std::io::Error> for InteractiveError {
    fn from(err: std::io::Error) -> Self {
        InteractiveError::Io(err)
    }
}

impl From<inquire::InquireError> for InteractiveError {
    fn from(err: inquire::InquireError) -> Self {
        InteractiveError::Prompt(err)
    }
}

impl From<crate::wcif::WcifLoadError> for InteractiveError {
    fn from(err: crate::wcif::WcifLoadError) -> Self {
        InteractiveError::Wcif(err)
    }
}

pub struct RawModeGuard;

impl RawModeGuard {
    pub fn enter() -> Result<Self, std::io::Error> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(stdout(), cursor::Show);
    }
}

#[derive(serde::Deserialize)]
pub struct WcaItem {
    pub id: String,
    pub name: String,
    pub country_iso2: Option<String>,
}
