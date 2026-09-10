pub mod events;
pub mod model;
pub mod planner;

pub use events::{RoundId, WcaEvent};
#[cfg(test)]
pub use model::WcaResult;
pub use model::{
    BlankScorecard, Competitor, CoverSheet, GroupNumber, RoundNumber, Scorecard, ScorecardItem,
    TimeLimitInfo,
};
pub use planner::{PlannerError, ScorecardPlanner};

#[cfg(test)]
pub use crate::wcif::WcaId;

#[cfg(test)]
mod tests;
