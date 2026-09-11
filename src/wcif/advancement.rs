use super::model::{Competition, Event, Round};

/// Result of calculating advancement blanks for a subsequent round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvancementResult {
    pub blank_count: usize,
    pub reason: String,
}

fn default_result(reason: impl Into<String>) -> AdvancementResult {
    AdvancementResult {
        blank_count: 16,
        reason: reason.into(),
    }
}

/// Calculator for round advancement rules (ranking cutoff, percentage advancement, etc.).
pub struct AdvancementCalculator;

impl AdvancementCalculator {
    /// Calculates the number of blank scorecards needed for a subsequent round based on WCA rules.
    pub fn calculate_blanks(comp: &Competition, event: &Event, round: &Round) -> AdvancementResult {
        let Some(prev_idx) = Self::find_previous_round_index(event, round) else {
            return default_result("no previous round found, defaulted to 16 blanks");
        };

        let prev = &event.rounds[prev_idx];
        let Some(ref cond) = prev.advancement_condition else {
            return default_result(
                "no previous round advancement condition found, defaulted to 16 blanks",
            );
        };

        match cond.condition_type.as_str() {
            "ranking" => Self::calculate_ranking_advancement(cond.value, comp, event, prev_idx),
            "percent" => Self::calculate_percent_advancement(cond.value, comp, event, prev_idx),
            "attemptResult" => default_result("cutoff-based advancement, defaulted to 16 blanks"),
            other => default_result(format!(
                "advancement condition type {other:?}, defaulted to 16 blanks"
            )),
        }
    }

    fn find_previous_round_index(event: &Event, round: &Round) -> Option<usize> {
        event
            .rounds
            .iter()
            .position(|r| r.id == round.id)
            .and_then(|idx| idx.checked_sub(1))
    }

    fn calculate_ranking_advancement(
        val: Option<usize>,
        comp: &Competition,
        event: &Event,
        prev_idx: usize,
    ) -> AdvancementResult {
        let limit = val.unwrap_or(16);
        let pool = Self::estimate_competitors_in_round(comp, event, prev_idx);
        let capped = limit.min(pool);
        AdvancementResult {
            blank_count: capped,
            reason: format!(
                "based on ranking advancement limit of top {capped} from previous round"
            ),
        }
    }

    fn calculate_percent_advancement(
        percent_val: Option<usize>,
        comp: &Competition,
        event: &Event,
        prev_idx: usize,
    ) -> AdvancementResult {
        let percent = percent_val.unwrap_or(75);
        let prev_pool = Self::estimate_competitors_in_round(comp, event, prev_idx);
        let calculated = (prev_pool * percent + 50) / 100;
        AdvancementResult {
            blank_count: calculated,
            reason: format!(
                "based on percentage advancement of {percent}% of ~{prev_pool} competitors ({calculated} blanks)"
            ),
        }
    }

    /// Estimates how many competitors will be in a given round (by index).
    /// Round 0 uses total accepted registrants; subsequent rounds chain advancement conditions.
    fn estimate_competitors_in_round(comp: &Competition, event: &Event, round_idx: usize) -> usize {
        if round_idx == 0 {
            return comp.count_competitors_for_event(&event.id);
        }
        let prev_round = &event.rounds[round_idx - 1];
        let prev_pool = Self::estimate_competitors_in_round(comp, event, round_idx - 1);
        match prev_round.advancement_condition.as_ref() {
            Some(cond) => match cond.condition_type.as_str() {
                "ranking" => cond.value.unwrap_or(16).min(prev_pool),
                "percent" => {
                    let percent = cond.value.unwrap_or(75);
                    (prev_pool * percent + 50) / 100
                }
                _ => prev_pool.min(16),
            },
            None => prev_pool.min(16),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::*;
    use crate::wcif::model::{AdvancementCondition, Person, Registration};

    fn make_test_comp() -> Competition {
        Competition {
            format_version: Some("1.0".to_owned()),
            id: "TestComp".to_owned(),
            name: "Test Competition".to_owned(),
            short_name: None,
            persons: vec![
                Person {
                    registrant_id: NonZeroUsize::new(1),
                    name: "Competitor 1".to_owned(),
                    wca_id: None,
                    country_iso2: None,
                    registration: Some(Registration {
                        id: NonZeroUsize::new(1),
                        status: Some("accepted".to_owned()),
                        event_ids: vec!["333".to_owned()],
                        is_competing: true,
                    }),
                    assignments: vec![],
                    personal_bests: vec![],
                },
                Person {
                    registrant_id: NonZeroUsize::new(2),
                    name: "Competitor 2".to_owned(),
                    wca_id: None,
                    country_iso2: None,
                    registration: Some(Registration {
                        id: NonZeroUsize::new(2),
                        status: Some("accepted".to_owned()),
                        event_ids: vec!["333".to_owned()],
                        is_competing: true,
                    }),
                    assignments: vec![],
                    personal_bests: vec![],
                },
                Person {
                    registrant_id: NonZeroUsize::new(3),
                    name: "Competitor 3".to_owned(),
                    wca_id: None,
                    country_iso2: None,
                    registration: Some(Registration {
                        id: NonZeroUsize::new(3),
                        status: Some("accepted".to_owned()),
                        event_ids: vec!["333".to_owned()],
                        is_competing: true,
                    }),
                    assignments: vec![],
                    personal_bests: vec![],
                },
                Person {
                    registrant_id: NonZeroUsize::new(4),
                    name: "Competitor 4".to_owned(),
                    wca_id: None,
                    country_iso2: None,
                    registration: Some(Registration {
                        id: NonZeroUsize::new(4),
                        status: Some("accepted".to_owned()),
                        event_ids: vec!["333".to_owned()],
                        is_competing: true,
                    }),
                    assignments: vec![],
                    personal_bests: vec![],
                },
            ],
            events: vec![],
            schedule: None,
            extensions: vec![],
        }
    }

    #[test]
    fn test_advancement_ranking() {
        let comp = make_test_comp(); // 4 competitors
        let event = Event {
            id: "333".to_owned(),
            rounds: vec![
                Round {
                    id: "333-r1".to_owned(),
                    format: Some("a".to_owned()),
                    time_limit: None,
                    cutoff: None,
                    advancement_condition: Some(AdvancementCondition {
                        condition_type: "ranking".to_owned(),
                        value: Some(3),
                    }),
                    scramble_group_count: 1,
                },
                Round {
                    id: "333-r2".to_owned(),
                    format: Some("a".to_owned()),
                    time_limit: None,
                    cutoff: None,
                    advancement_condition: None,
                    scramble_group_count: 1,
                },
            ],
            competitor_limit: None,
            qualification: None,
        };

        let result = AdvancementCalculator::calculate_blanks(&comp, &event, &event.rounds[1]);
        assert_eq!(result.blank_count, 3);
        assert!(result.reason.contains("top 3"));

        // When ranking limit exceeds pool size, cap at pool size
        let event_over = Event {
            id: "333".to_owned(),
            rounds: vec![
                Round {
                    id: "333-r1".to_owned(),
                    format: Some("a".to_owned()),
                    time_limit: None,
                    cutoff: None,
                    advancement_condition: Some(AdvancementCondition {
                        condition_type: "ranking".to_owned(),
                        value: Some(12),
                    }),
                    scramble_group_count: 1,
                },
                Round {
                    id: "333-r2".to_owned(),
                    format: Some("a".to_owned()),
                    time_limit: None,
                    cutoff: None,
                    advancement_condition: None,
                    scramble_group_count: 1,
                },
            ],
            competitor_limit: None,
            qualification: None,
        };

        let result_capped =
            AdvancementCalculator::calculate_blanks(&comp, &event_over, &event_over.rounds[1]);
        assert_eq!(result_capped.blank_count, 4); // capped at 4 competitors
    }

    #[test]
    fn test_advancement_percent() {
        let comp = make_test_comp();
        let event = Event {
            id: "333".to_owned(),
            rounds: vec![
                Round {
                    id: "333-r1".to_owned(),
                    format: Some("a".to_owned()),
                    time_limit: None,
                    cutoff: None,
                    advancement_condition: Some(AdvancementCondition {
                        condition_type: "percent".to_owned(),
                        value: Some(50),
                    }),
                    scramble_group_count: 1,
                },
                Round {
                    id: "333-r2".to_owned(),
                    format: Some("a".to_owned()),
                    time_limit: None,
                    cutoff: None,
                    advancement_condition: None,
                    scramble_group_count: 1,
                },
            ],
            competitor_limit: None,
            qualification: None,
        };

        let result = AdvancementCalculator::calculate_blanks(&comp, &event, &event.rounds[1]);
        // 50% of 4 competitors = 2
        assert_eq!(result.blank_count, 2);
        assert!(result.reason.contains("50%"));
    }

    #[test]
    fn test_advancement_fallbacks() {
        let comp = make_test_comp();
        let event_no_condition = Event {
            id: "333".to_owned(),
            rounds: vec![
                Round {
                    id: "333-r1".to_owned(),
                    format: Some("a".to_owned()),
                    time_limit: None,
                    cutoff: None,
                    advancement_condition: None,
                    scramble_group_count: 1,
                },
                Round {
                    id: "333-r2".to_owned(),
                    format: Some("a".to_owned()),
                    time_limit: None,
                    cutoff: None,
                    advancement_condition: None,
                    scramble_group_count: 1,
                },
            ],
            competitor_limit: None,
            qualification: None,
        };

        let result_no_cond = AdvancementCalculator::calculate_blanks(
            &comp,
            &event_no_condition,
            &event_no_condition.rounds[1],
        );
        assert_eq!(result_no_cond.blank_count, 16);

        let result_first_round = AdvancementCalculator::calculate_blanks(
            &comp,
            &event_no_condition,
            &event_no_condition.rounds[0],
        );
        assert_eq!(result_first_round.blank_count, 16);
    }
}
