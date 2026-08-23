use super::events::event_name_by_id;
use super::model::ScorecardItem;
use crate::wcif::{
    AdvancementCalculator, Competition, Cutoff, Event, Person, Round, ScheduledActivityInfo,
    TimeLimit,
};
use rustc_hash::FxHashMap;
use std::error::Error;

/// Generation target representing an event and round to produce scorecards for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationTarget {
    pub event_id: String,
    pub round_id: String,
    pub round_number: usize,
    pub is_open_round: bool,
}

fn parse_event_arg(arg: &str) -> (String, usize) {
    let mut parts = arg.split('-');
    let event_id = parts.next().unwrap_or(arg).to_string();
    let round_num = match parts.next() {
        Some(r) => match r.trim_start_matches('r').parse::<usize>() {
            Ok(n) => n,
            Err(_) => {
                eprintln!(
                    "Warning: unrecognized round specifier '{}' in '{}', defaulting to round 1",
                    r, arg
                );
                1
            }
        },
        None => 1,
    };
    if parts.next().is_some() {
        eprintln!(
            "Warning: ignoring extra segments in event argument '{}'",
            arg
        );
    }
    (event_id, round_num)
}

/// Formats centiseconds into a human-readable time string (e.g. "1:30.50").
/// Returns "None" for values ≤ 0 (covers WCA sentinels: -1 = DNF, -2 = DNS).
fn format_centiseconds(centis: isize) -> String {
    if centis <= 0 {
        return "None".to_string();
    }
    let total_seconds = centis / 100;
    let cs = centis % 100;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    if minutes > 0 {
        if cs > 0 {
            format!("{}:{:02}.{:02}", minutes, seconds, cs)
        } else {
            format!("{}:{:02}.00", minutes, seconds)
        }
    } else {
        format!("{}.{:02}", seconds, cs)
    }
}

pub fn format_limit_and_cutoff(
    time_limit: Option<&TimeLimit>,
    cutoff: Option<&Cutoff>,
) -> Option<String> {
    let cutoff_part = cutoff.map(|c| {
        format!(
            "Cutoff: < {} ({} att)",
            format_centiseconds(c.attempt_result),
            c.number_of_attempts
        )
    });

    let time_limit_part = time_limit.map(|tl| {
        let time_str = format_centiseconds(tl.centiseconds);
        if let Some(ref ids) = tl.cumulative_round_ids
            && !ids.is_empty()
        {
            format!("Time limit: {} cumulative", time_str)
        } else {
            format!("Time limit: {}", time_str)
        }
    });

    match (cutoff_part, time_limit_part) {
        (Some(c), Some(t)) => Some(format!("{}  |  {}", c, t)),
        (Some(c), None) => Some(c),
        (None, Some(t)) => Some(t),
        (None, None) => None,
    }
}

fn find_event_and_round<'a>(
    comp: &'a Competition,
    target: &GenerationTarget,
) -> Result<(&'a Event, &'a Round), Box<dyn Error>> {
    let event = comp
        .events
        .iter()
        .find(|e| e.id == target.event_id)
        .ok_or_else(|| format!("Event {:?} not found in WCIF", target.event_id))?;

    let round = event
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

    Ok((event, round))
}

fn resolve_assignment<'a>(
    person: &'a Person,
    activity_map: &FxHashMap<usize, ScheduledActivityInfo<'a>>,
    target_prefix: &str,
) -> (usize, Option<usize>, Option<&'a str>) {
    for assign in &person.assignments {
        if let Some(info) = activity_map.get(&assign.activity_id)
            && let Some(stripped) = info.activity_code.strip_prefix(target_prefix)
        {
            let group_num: usize = stripped.parse().unwrap_or_else(|_| {
                eprintln!(
                    "Warning: non-numeric group suffix '{}' in activity code, defaulting to group 1",
                    stripped
                );
                1
            });
            return (group_num, assign.station_number, info.room_name);
        }
    }
    (1, None, None)
}

#[allow(clippy::too_many_arguments)]
fn plan_open_round<'a>(
    comp: &'a Competition,
    target: &GenerationTarget,
    matched_event: &'a Event,
    matched_round: &'a Round,
    event_name: &'static str,
    attempt_count: usize,
    activity_map: &FxHashMap<usize, ScheduledActivityInfo<'a>>,
    scorecards: &mut Vec<ScorecardItem<'a>>,
) {
    let target_prefix = format!("{}-g", target.round_id);
    let comp_name = comp.display_name();
    let time_limit_info = format_limit_and_cutoff(
        matched_round.time_limit.as_ref(),
        matched_round.cutoff.as_ref(),
    );
    let mut count = 0;
    let mut names = Vec::new();

    for person in comp.accepted_competitors_for_event(&matched_event.id) {
        count += 1;
        if count <= 5 {
            names.push(person.name.as_str());
        }

        let (group_num, station_num, stage_name) =
            resolve_assignment(person, activity_map, &target_prefix);

        scorecards.push(ScorecardItem {
            scorecard_number: 0,
            station_number: station_num,
            competition_name: comp_name,
            event_id: &matched_event.id,
            event_name,
            round_number: target.round_number,
            group_number: group_num,
            stage_name,
            competitor_name: person.name.as_str(),
            registrant_id: person.registrant_id(),
            wca_id: person.wca_id.as_deref(),
            attempt_count,
            time_limit_info: time_limit_info.clone(),
            is_blank: false,
        });
    }

    println!(
        "[{event_id} Round {round_num}] (Open Round) -> Generating scorecards for {count} accepted competitors",
        event_id = target.event_id,
        round_num = target.round_number,
    );
    if count <= 5 && count > 0 {
        println!("   Competitors: {}", names.join(", "));
    }
}

#[allow(clippy::too_many_arguments)]
fn plan_subsequent_round<'a>(
    comp: &'a Competition,
    target: &GenerationTarget,
    matched_event: &'a Event,
    matched_round: &'a Round,
    event_name: &'static str,
    attempt_count: usize,
    activity_map: &FxHashMap<usize, ScheduledActivityInfo<'a>>,
    scorecards: &mut Vec<ScorecardItem<'a>>,
) {
    let adv_result = AdvancementCalculator::calculate_blanks(comp, matched_event, matched_round);
    println!(
        "[{event_id} Round {round_num}] (Subsequent Round) -> Generating {blank_count} blank scorecards ({reason})",
        event_id = target.event_id,
        round_num = target.round_number,
        blank_count = adv_result.blank_count,
        reason = adv_result.reason
    );

    let round_stage = activity_map
        .values()
        .find(|info| {
            info.activity_code == target.round_id
                || info
                    .activity_code
                    .starts_with(&format!("{}-g", target.round_id))
        })
        .and_then(|info| info.room_name);

    let comp_name = comp.display_name();
    let time_limit_info = format_limit_and_cutoff(
        matched_round.time_limit.as_ref(),
        matched_round.cutoff.as_ref(),
    );
    for _ in 0..adv_result.blank_count {
        scorecards.push(ScorecardItem {
            scorecard_number: 0,
            station_number: None,
            competition_name: comp_name,
            event_id: &matched_event.id,
            event_name,
            round_number: target.round_number,
            group_number: 1,
            stage_name: round_stage,
            competitor_name: "",
            registrant_id: None,
            wca_id: None,
            attempt_count,
            time_limit_info: time_limit_info.clone(),
            is_blank: true,
        });
    }
}

/// Planner responsible for extracting competitor assignments, calculating advancement blanks, and sequencing scorecards.
pub struct ScorecardPlanner;

impl ScorecardPlanner {
    /// Resolves generation targets from CLI arguments (or all open rounds if none specified).
    /// Omits events that do not use scorecards (such as 3x3x3 Fewest Moves - 333fm).
    pub fn resolve_targets(
        comp: &Competition,
        requested_events: &[String],
    ) -> Vec<GenerationTarget> {
        if requested_events.is_empty() {
            return comp
                .events
                .iter()
                .filter(|e| e.id != "333fm")
                .filter_map(|e| e.rounds.first().map(|r| (e, r)))
                .map(|(event, first_round)| GenerationTarget {
                    event_id: event.id.clone(),
                    round_id: first_round.id.clone(),
                    round_number: 1,
                    is_open_round: true,
                })
                .collect();
        }

        requested_events
            .iter()
            .filter_map(|arg| {
                let (event_id, round_num) = parse_event_arg(arg);
                if event_id == "333fm" {
                    println!(
                        "Note: Skipping '333fm' (3x3x3 Fewest Moves) as it does not use scorecards."
                    );
                    return None;
                }
                Some(GenerationTarget {
                    round_id: format!("{}-r{}", event_id, round_num),
                    event_id,
                    round_number: round_num,
                    // Heuristic: Round 1 is treated as an open round (named scorecards).
                    // In rare cases with qualification rounds, R1 could be a subsequent round,
                    // but WCIF doesn't distinguish this reliably without results data.
                    is_open_round: round_num == 1,
                })
            })
            .collect()
    }

    /// Plans and builds all required scorecards for the competition given the requested events.
    pub fn plan<'a>(
        comp: &'a Competition,
        requested_events: &[String],
    ) -> Result<Vec<ScorecardItem<'a>>, Box<dyn Error>> {
        let targets = Self::resolve_targets(comp, requested_events);
        let activity_map = comp.build_activity_schedule_map();
        let mut scorecards = Vec::with_capacity(comp.persons.len() * targets.len().max(1));

        println!("\n--- Scorecard Generation Plan ---");
        for target in &targets {
            let (matched_event, matched_round) = find_event_and_round(comp, target)?;
            let event_name = event_name_by_id(&target.event_id).ok_or_else(|| {
                format!("unknown or unsupported WCA event ID: '{}'", target.event_id)
            })?;
            let attempt_count = matched_round.attempt_count();

            if target.is_open_round {
                plan_open_round(
                    comp,
                    target,
                    matched_event,
                    matched_round,
                    event_name,
                    attempt_count,
                    &activity_map,
                    &mut scorecards,
                );
            } else {
                plan_subsequent_round(
                    comp,
                    target,
                    matched_event,
                    matched_round,
                    event_name,
                    attempt_count,
                    &activity_map,
                    &mut scorecards,
                );
            }
        }
        println!("---------------------------------");

        for (idx, card) in scorecards.iter_mut().enumerate() {
            card.scorecard_number = idx + 1;
        }

        Ok(scorecards)
    }
}
