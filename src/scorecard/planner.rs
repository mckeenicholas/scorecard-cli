use super::events::event_name_by_id;
use super::model::ScorecardItem;
use crate::wcif::{AdvancementCalculator, Competition, Event, Round};
use std::error::Error;

/// Generation target representing an event and round to produce scorecards for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationTarget {
    pub event_id: String,
    pub round_id: String,
    pub round_number: usize,
    pub is_open_round: bool,
}

/// Planner responsible for extracting competitor assignments, calculating advancement blanks, and sequencing scorecards.
pub struct ScorecardPlanner;

impl ScorecardPlanner {
    /// Resolves generation targets from CLI arguments (or all open rounds if none specified).
    pub fn resolve_targets(
        comp: &Competition,
        requested_events: &[String],
    ) -> Vec<GenerationTarget> {
        let mut targets = Vec::new();

        if requested_events.is_empty() {
            // Default: Generate all open rounds (first round of each event)
            for event in &comp.events {
                if let Some(first_round) = event.rounds.first() {
                    targets.push(GenerationTarget {
                        event_id: event.id.clone(),
                        round_id: first_round.id.clone(),
                        round_number: 1,
                        is_open_round: true,
                    });
                }
            }
        } else {
            // Parse requested events/rounds from CLI arguments
            for arg in requested_events {
                let parts: Vec<&str> = arg.split('-').collect();
                let event_id = parts[0].to_string();
                let mut round_num = 1;
                let mut is_open = true;

                if parts.len() > 1 {
                    let round_str = parts[1].trim_start_matches('r');
                    match round_str.parse::<usize>() {
                        Ok(num) => {
                            round_num = num;
                        }
                        Err(_) => {
                            eprintln!(
                                "Warning: failed to parse round number in {:?}, defaulting to round 1",
                                arg
                            );
                            round_num = 1;
                        }
                    }
                    if round_num > 1 {
                        is_open = false;
                    }
                }

                targets.push(GenerationTarget {
                    event_id: event_id.clone(),
                    round_id: format!("{}-r{}", event_id, round_num),
                    round_number: round_num,
                    is_open_round: is_open,
                });
            }
        }

        targets
    }

    /// Plans and builds all required scorecards for the competition given the requested events.
    pub fn plan<'a>(
        comp: &'a Competition,
        requested_events: &[String],
    ) -> Result<Vec<ScorecardItem<'a>>, Box<dyn Error>> {
        let targets = Self::resolve_targets(comp, requested_events);
        let activity_map = comp.build_activity_map();

        let estimated_capacity = comp.persons.len() * targets.len().max(1);
        let mut scorecards = Vec::with_capacity(estimated_capacity);

        println!("\n--- Scorecard Generation Plan ---");
        for target in &targets {
            // Find event in WCIF
            let matched_event: &'a Event = comp
                .events
                .iter()
                .find(|e| e.id == target.event_id)
                .ok_or_else(|| format!("Event {:?} not found in WCIF", target.event_id))?;

            // Find round in WCIF
            let matched_round: &'a Round = matched_event
                .rounds
                .iter()
                .find(|r| {
                    r.id == target.round_id
                        || r.id == format!("{}-r{}", target.event_id, target.round_number)
                })
                .ok_or_else(|| {
                    format!(
                        "Round {:?} for event {:?} not found in WCIF schedule",
                        target.round_id, target.event_id
                    )
                })?;

            let attempt_count = matched_round.attempt_count();
            let event_display_name = event_name_by_id(&target.event_id)?;
            let comp_name = comp.display_name();

            if target.is_open_round {
                let mut competitor_count = 0;
                let mut competitor_names = Vec::new();
                let target_prefix = format!("{}-g", target.round_id);

                for person in &comp.persons {
                    if let Some(ref reg) = person.registration
                        && reg.status.as_deref() == Some("accepted")
                        && reg.is_competing
                        && reg.event_ids.iter().any(|id| id == &target.event_id)
                    {
                        competitor_count += 1;
                        if competitor_count <= 5 {
                            competitor_names.push(person.name.as_str());
                        }

                        // Resolve group and station numbers from schedule assignments
                        let mut group_num = 1;
                        let mut station_num = None;

                        for assign in &person.assignments {
                            if let Some(act_code) = activity_map.get(&assign.activity_id)
                                && let Some(stripped) = act_code.strip_prefix(&target_prefix)
                            {
                                if let Ok(g) = stripped.parse::<usize>() {
                                    group_num = g;
                                }
                                station_num = assign.station_number;
                                break;
                            }
                        }

                        scorecards.push(ScorecardItem {
                            scorecard_number: 0,
                            station_number: station_num,
                            competition_name: comp_name,
                            event_id: &matched_event.id,
                            event_name: event_display_name,
                            round_number: target.round_number,
                            group_number: group_num,
                            competitor_name: person.name.as_str(),
                            registrant_id: person.registrant_id,
                            wca_id: person.wca_id.as_deref(),
                            attempt_count,
                            is_blank: false,
                        });
                    }
                }

                println!(
                    "[{event_id} Round {round_num}] (Open Round) -> Generating scorecards for {competitor_count} accepted competitors",
                    event_id = target.event_id,
                    round_num = target.round_number,
                    competitor_count = competitor_count
                );
                if competitor_count <= 5 && competitor_count > 0 {
                    println!("   Competitors: {}", competitor_names.join(", "));
                }
            } else {
                let adv_result =
                    AdvancementCalculator::calculate_blanks(comp, matched_event, matched_round);

                println!(
                    "[{event_id} Round {round_num}] (Subsequent Round) -> Generating {blank_count} blank scorecards ({reason})",
                    event_id = target.event_id,
                    round_num = target.round_number,
                    blank_count = adv_result.blank_count,
                    reason = adv_result.reason
                );

                for _ in 0..adv_result.blank_count {
                    scorecards.push(ScorecardItem {
                        scorecard_number: 0,
                        station_number: None,
                        competition_name: comp_name,
                        event_id: &matched_event.id,
                        event_name: event_display_name,
                        round_number: target.round_number,
                        group_number: 1,
                        competitor_name: "",
                        registrant_id: None,
                        wca_id: None,
                        attempt_count,
                        is_blank: true,
                    });
                }
            }
        }
        println!("---------------------------------");

        // Sequence scorecard numbers 1..=N
        for (idx, card) in scorecards.iter_mut().enumerate() {
            card.scorecard_number = idx + 1;
        }

        Ok(scorecards)
    }
}
