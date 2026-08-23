use super::events::event_name_by_id;
use super::model::{PlannedRoundSummary, ScorecardItem, ScorecardPlan, TimeLimitInfo};
use crate::wcif::{
    AdvancementCalculator, Competition, Event, Person, Round, ScheduledActivityInfo,
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
        Some(r) => r.trim_start_matches('r').parse::<usize>().unwrap_or(1),
        None => 1,
    };
    (event_id, round_num)
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
            let group_num: usize = stripped.parse().unwrap_or(1);
            return (group_num, assign.station_number, info.room_name);
        }
    }
    (1, None, None)
}

struct RoundPlanningContext<'a, 'b> {
    comp: &'a Competition,
    target: &'b GenerationTarget,
    event: &'a Event,
    round: &'a Round,
    event_name: &'static str,
    attempt_count: usize,
    activity_map: &'b FxHashMap<usize, ScheduledActivityInfo<'a>>,
    cover_sheets: bool,
}

fn plan_open_round<'a>(ctx: &RoundPlanningContext<'a, '_>, plan: &mut ScorecardPlan<'a>) {
    let target_prefix = format!("{}-g", ctx.target.round_id);
    let comp_name = ctx.comp.display_name();
    let time_limit_info =
        TimeLimitInfo::from_wcif(ctx.round.time_limit.as_ref(), ctx.round.cutoff.as_ref());
    let mut count = 0;
    let mut sample_names = Vec::new();

    // Group cards deterministically by (stage_name, group_number)
    let mut groups: std::collections::BTreeMap<(Option<&'a str>, usize), Vec<ScorecardItem<'a>>> =
        std::collections::BTreeMap::new();

    for person in ctx.comp.accepted_competitors_for_event(&ctx.event.id) {
        count += 1;
        if count <= 5 {
            sample_names.push(person.name.clone());
        }

        let (group_num, station_num, stage_name) =
            resolve_assignment(person, ctx.activity_map, &target_prefix);

        let item = ScorecardItem {
            scorecard_number: 0,
            station_number: station_num,
            competition_name: comp_name,
            event_id: &ctx.event.id,
            event_name: ctx.event_name,
            round_number: ctx.target.round_number,
            group_number: group_num,
            stage_name,
            competitor_name: person.name.as_str(),
            registrant_id: person.registrant_id(),
            wca_id: person.wca_id.as_deref(),
            attempt_count: ctx.attempt_count,
            time_limit_info,
            is_blank: false,
            is_cover_sheet: false,
            total_group_cards: 0,
        };

        groups
            .entry((stage_name, group_num))
            .or_default()
            .push(item);
    }

    for ((stage_name, group_num), mut card_list) in groups {
        // Sort cards within group by station_number, then competitor_name
        card_list.sort_by(|a, b| {
            a.station_number
                .cmp(&b.station_number)
                .then_with(|| a.competitor_name.cmp(b.competitor_name))
        });

        if ctx.cover_sheets && !card_list.is_empty() {
            plan.items.push(ScorecardItem {
                scorecard_number: 0,
                station_number: None,
                competition_name: comp_name,
                event_id: &ctx.event.id,
                event_name: ctx.event_name,
                round_number: ctx.target.round_number,
                group_number: group_num,
                stage_name,
                competitor_name: "",
                registrant_id: None,
                wca_id: None,
                attempt_count: ctx.attempt_count,
                time_limit_info: None,
                is_blank: false,
                is_cover_sheet: true,
                total_group_cards: card_list.len(),
            });
        }

        plan.items.extend(card_list);
    }

    plan.summaries.push(PlannedRoundSummary::OpenRound {
        event_id: ctx.target.event_id.clone(),
        round_number: ctx.target.round_number,
        competitor_count: count,
        sample_competitor_names: sample_names,
    });
}

fn plan_subsequent_round<'a>(ctx: &RoundPlanningContext<'a, '_>, plan: &mut ScorecardPlan<'a>) {
    let adv_result = AdvancementCalculator::calculate_blanks(ctx.comp, ctx.event, ctx.round);

    let round_stage = ctx
        .activity_map
        .values()
        .find(|info| {
            info.activity_code == ctx.target.round_id
                || info
                    .activity_code
                    .starts_with(&format!("{}-g", ctx.target.round_id))
        })
        .and_then(|info| info.room_name);

    let comp_name = ctx.comp.display_name();
    let time_limit_info =
        TimeLimitInfo::from_wcif(ctx.round.time_limit.as_ref(), ctx.round.cutoff.as_ref());

    if ctx.cover_sheets && adv_result.blank_count > 0 {
        plan.items.push(ScorecardItem {
            scorecard_number: 0,
            station_number: None,
            competition_name: comp_name,
            event_id: &ctx.event.id,
            event_name: ctx.event_name,
            round_number: ctx.target.round_number,
            group_number: 1,
            stage_name: round_stage,
            competitor_name: "",
            registrant_id: None,
            wca_id: None,
            attempt_count: ctx.attempt_count,
            time_limit_info: None,
            is_blank: false,
            is_cover_sheet: true,
            total_group_cards: adv_result.blank_count,
        });
    }

    for _ in 0..adv_result.blank_count {
        plan.items.push(ScorecardItem {
            scorecard_number: 0,
            station_number: None,
            competition_name: comp_name,
            event_id: &ctx.event.id,
            event_name: ctx.event_name,
            round_number: ctx.target.round_number,
            group_number: 1,
            stage_name: round_stage,
            competitor_name: "",
            registrant_id: None,
            wca_id: None,
            attempt_count: ctx.attempt_count,
            time_limit_info,
            is_blank: true,
            is_cover_sheet: false,
            total_group_cards: 0,
        });
    }

    plan.summaries.push(PlannedRoundSummary::SubsequentRound {
        event_id: ctx.target.event_id.clone(),
        round_number: ctx.target.round_number,
        blank_count: adv_result.blank_count,
        reason: adv_result.reason,
    });
}

/// Planner responsible for extracting competitor assignments, calculating advancement blanks, and sequencing scorecards.
pub struct ScorecardPlanner;

impl ScorecardPlanner {
    /// Resolves generation targets from CLI arguments (or all open rounds if none specified), returning targets and any diagnostic notes.
    /// Omits events that do not use scorecards (such as 3x3x3 Fewest Moves - 333fm).
    pub fn resolve_targets(
        comp: &Competition,
        requested_events: &[String],
    ) -> (Vec<GenerationTarget>, Vec<String>) {
        let mut notes = Vec::new();

        if requested_events.is_empty() {
            let targets = comp
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
            return (targets, notes);
        }

        let targets = requested_events
            .iter()
            .filter_map(|arg| {
                let (event_id, round_num) = parse_event_arg(arg);
                if event_id == "333fm" {
                    notes.push(
                        "Skipping '333fm' (3x3x3 Fewest Moves) as it does not use scorecards."
                            .to_string(),
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
            .collect();

        (targets, notes)
    }

    /// Plans and builds all required scorecards for the competition given the requested events and cover sheet option.
    pub fn plan<'a>(
        comp: &'a Competition,
        requested_events: &[String],
        cover_sheets: bool,
    ) -> Result<ScorecardPlan<'a>, Box<dyn Error>> {
        let (targets, notes) = Self::resolve_targets(comp, requested_events);
        let activity_map = comp.build_activity_schedule_map();
        let mut plan = ScorecardPlan::new(notes);

        for target in &targets {
            Self::plan_target_round(comp, target, &activity_map, cover_sheets, &mut plan)?;
        }

        plan.assign_scorecard_numbers();
        Ok(plan)
    }

    fn plan_target_round<'a>(
        comp: &'a Competition,
        target: &GenerationTarget,
        activity_map: &FxHashMap<usize, ScheduledActivityInfo<'a>>,
        cover_sheets: bool,
        plan: &mut ScorecardPlan<'a>,
    ) -> Result<(), Box<dyn Error>> {
        let (event, round) = find_event_and_round(comp, target)?;
        let event_name = event_name_by_id(&target.event_id)
            .ok_or_else(|| format!("unknown or unsupported WCA event ID: '{}'", target.event_id))?;
        let attempt_count = round.attempt_count();

        let ctx = RoundPlanningContext {
            comp,
            target,
            event,
            round,
            event_name,
            attempt_count,
            activity_map,
            cover_sheets,
        };

        if target.is_open_round {
            plan_open_round(&ctx, plan);
        } else {
            plan_subsequent_round(&ctx, plan);
        }
        Ok(())
    }
}
