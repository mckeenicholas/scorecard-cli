pub mod events;
pub mod model;
pub mod planner;

#[cfg(test)]
pub use crate::wcif::WcaId;
pub use events::WcaEvent;
pub use model::Competitor;
#[cfg(test)]
pub use model::WcaResult;
pub use model::{BlankScorecard, CoverSheet, Scorecard, ScorecardItem, TimeLimitInfo};
pub use planner::{PlannerError, ScorecardPlanner};

#[cfg(test)]
mod tests;
