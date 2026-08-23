use super::model::{Competition, Event, Round};

/// Result of calculating advancement blanks for a subsequent round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvancementResult {
    pub blank_count: usize,
    pub reason: String,
}

/// Calculator for round advancement rules (ranking cutoff, percentage advancement, etc.).
pub struct AdvancementCalculator;

impl AdvancementCalculator {
    /// Calculates the number of blank scorecards needed for a subsequent round based on WCA rules.
    pub fn calculate_blanks(comp: &Competition, event: &Event, round: &Round) -> AdvancementResult {
        // Find previous round of this event to calculate advancement
        let prev_round = event
            .rounds
            .iter()
            .position(|r| r.id == round.id)
            .and_then(|idx| {
                if idx > 0 {
                    event.rounds.get(idx - 1)
                } else {
                    None
                }
            });

        if let Some(prev) = prev_round {
            if let Some(ref cond) = prev.advancement_condition {
                match cond.condition_type.as_str() {
                    "ranking" => {
                        let val = cond.value.unwrap_or(16.0) as usize;
                        AdvancementResult {
                            blank_count: val,
                            reason: format!(
                                "based on ranking advancement limit of top {} from previous round",
                                val
                            ),
                        }
                    }
                    "percent" => {
                        let percent = cond.value.unwrap_or(75.0);
                        let prev_competitors = comp.count_competitors_for_event(&event.id);
                        let calculated =
                            (prev_competitors as f64 * percent / 100.0).round() as usize;
                        AdvancementResult {
                            blank_count: calculated,
                            reason: format!(
                                "based on percentage advancement limit of {}% of {} competitors ({} blanks)",
                                percent, prev_competitors, calculated
                            ),
                        }
                    }
                    "attemptResult" => AdvancementResult {
                        blank_count: 16,
                        reason: "cutoff-based advancement, defaulted to 16 blanks".to_string(),
                    },
                    _ => AdvancementResult {
                        blank_count: 16,
                        reason: format!(
                            "advancement condition type {:?}, defaulted to 16 blanks",
                            cond.condition_type
                        ),
                    },
                }
            } else {
                AdvancementResult {
                    blank_count: 16,
                    reason: "no previous round advancement condition found, defaulted to 16 blanks"
                        .to_string(),
                }
            }
        } else {
            AdvancementResult {
                blank_count: 16,
                reason: "no previous round found, defaulted to 16 blanks".to_string(),
            }
        }
    }
}
