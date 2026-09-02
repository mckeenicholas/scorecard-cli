use super::events::event_name_by_id;
use super::model::{PlannedRoundSummary, ScorecardItem, ScorecardPlan, TimeLimitInfo};
use crate::options::CoverSheetBy;
use crate::wcif::{
    AdvancementCalculator, Competition, Event, Person, Round, ScheduledActivityInfo,
};
use rustc_hash::FxHashMap;
use std::collections::BTreeMap;
use std::error::Error;

/// Generation target representing an event and round to produce scorecards for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationTarget {
    pub event_id: String,
    pub round_id: String,
    pub round_number: usize,
    pub is_open_round: bool,
}

fn parse_event_arg(arg: &str) -> (String, Option<usize>) {
    let mut parts = arg.split('-');
    let event_id = parts.next().unwrap_or(arg).to_string();
    let round_num = parts
        .next()
        .and_then(|r| r.trim_start_matches('r').parse::<usize>().ok());
    (event_id, round_num)
}

fn has_competitor_assignments(
    comp: &Competition,
    activity_map: &FxHashMap<usize, ScheduledActivityInfo<'_>>,
    round_id: &str,
) -> bool {
    let target_prefix = format!("{}-g", round_id);
    for person in &comp.persons {
        for assign in &person.assignments {
            if assign.assignment_code.as_deref() == Some("competitor")
                || assign.assignment_code.is_none()
            {
                if let Some(info) = activity_map.get(&assign.activity_id) {
                    if info.activity_code.starts_with(&target_prefix)
                        || info.activity_code == round_id
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
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
    round_id: &str,
) -> Option<(usize, Option<usize>, Option<&'a str>)> {
    for assign in &person.assignments {
        if assign.assignment_code.as_deref() == Some("competitor")
            || assign.assignment_code.is_none()
        {
            if let Some(info) = activity_map.get(&assign.activity_id) {
                if let Some(stripped) = info.activity_code.strip_prefix(target_prefix) {
                    let group_num: usize = stripped.parse().unwrap_or(1);
                    return Some((group_num, assign.station_number, info.room_name));
                } else if info.activity_code == round_id {
                    return Some((1, assign.station_number, info.room_name));
                }
            }
        }
    }
    None
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
    cover_sheets_by: &'b [CoverSheetBy],
    print_stations: bool,
}

type GroupKey<'a> = (usize, Option<&'a str>);
type GroupMap<'a> = BTreeMap<GroupKey<'a>, Vec<ScorecardItem<'a>>>;

fn collect_open_round_competitors<'a>(
    ctx: &RoundPlanningContext<'a, '_>,
) -> (GroupMap<'a>, usize, Vec<String>) {
    let target_prefix = format!("{}-g", ctx.target.round_id);
    let comp_name = ctx.comp.display_name();
    let time_limit_info =
        TimeLimitInfo::from_wcif(ctx.round.time_limit.as_ref(), ctx.round.cutoff.as_ref());
    let mut count = 0;
    let mut sample_names = Vec::new();
    let mut groups = GroupMap::new();

    for person in ctx.comp.accepted_competitors_for_event(&ctx.event.id) {
        let assignment = resolve_assignment(
            person,
            ctx.activity_map,
            &target_prefix,
            &ctx.target.round_id,
        );

        let (group_num, station_num, stage_name) = match assignment {
            Some(a) => a,
            None => {
                if ctx.target.round_number == 1 {
                    (1, None, None)
                } else {
                    continue;
                }
            }
        };

        let station_num = if ctx.print_stations {
            station_num
        } else {
            None
        };

        count += 1;
        if count <= 5 {
            sample_names.push(person.name.clone());
        }

        let item = ScorecardItem::competitor(
            comp_name,
            &ctx.event.id,
            ctx.event_name,
            ctx.target.round_number,
            group_num,
            stage_name,
            person.name.as_str(),
            person.registrant_id(),
            person.wca_id.as_deref(),
            station_num,
            ctx.attempt_count,
            time_limit_info,
        );

        groups
            .entry((group_num, stage_name))
            .or_default()
            .push(item);
    }

    (groups, count, sample_names)
}

/// Sorts cards within a group:
/// 1. Cards with station numbers come first, sorted by station number ascending.
/// 2. Cards without station numbers (or sharing the same station number) are sorted alphabetically by competitor name.
pub fn sort_group_cards(cards: &mut [ScorecardItem<'_>]) {
    cards.sort_by(|a, b| match (a.station_number, b.station_number) {
        (Some(s_a), Some(s_b)) => s_a
            .cmp(&s_b)
            .then_with(|| a.competitor_name.cmp(b.competitor_name)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.competitor_name.cmp(b.competitor_name),
    });
}

fn plan_open_round<'a>(ctx: &RoundPlanningContext<'a, '_>, plan: &mut ScorecardPlan<'a>) {
    let (groups, count, sample_names) = collect_open_round_competitors(ctx);

    if count == 0 && ctx.target.round_number > 1 {
        plan_subsequent_round(ctx, plan);
        return;
    }

    let has_round = ctx.cover_sheets && ctx.cover_sheets_by.contains(&CoverSheetBy::Round);
    let has_group = ctx.cover_sheets && ctx.cover_sheets_by.contains(&CoverSheetBy::Group);
    let has_stage = ctx.cover_sheets && ctx.cover_sheets_by.contains(&CoverSheetBy::Stage);

    // Pre-calculate card counts per group (across all stages)
    let mut group_totals: BTreeMap<usize, usize> = BTreeMap::new();
    for ((group_num, _), cards) in &groups {
        *group_totals.entry(*group_num).or_default() += cards.len();
    }

    // Tier 1 (Highest): Round cover sheet for entire round of the event
    if has_round && count > 0 {
        plan.items.push(ScorecardItem::cover_sheet(
            ctx.comp.display_name(),
            &ctx.event.id,
            ctx.event_name,
            ctx.target.round_number,
            0,
            None,
            ctx.attempt_count,
            count,
        ));
    }

    let mut current_group = None;

    for ((group_num, stage_name), mut card_list) in groups {
        sort_group_cards(&mut card_list);

        // Tier 2: Group cover sheet for the entire group across all stages
        if has_group && current_group != Some(group_num) {
            let group_total = group_totals
                .get(&group_num)
                .copied()
                .unwrap_or(card_list.len());
            plan.items.push(ScorecardItem::cover_sheet(
                ctx.comp.display_name(),
                &ctx.event.id,
                ctx.event_name,
                ctx.target.round_number,
                group_num,
                None,
                ctx.attempt_count,
                group_total,
            ));
            current_group = Some(group_num);
        }

        // Tier 3: Stage cover sheet (per group on each stage)
        // If stage is None and group cover sheet was already emitted, avoid duplicate cover sheet
        if has_stage && (stage_name.is_some() || !has_group) {
            plan.items.push(ScorecardItem::cover_sheet(
                ctx.comp.display_name(),
                &ctx.event.id,
                ctx.event_name,
                ctx.target.round_number,
                group_num,
                stage_name,
                ctx.attempt_count,
                card_list.len(),
            ));
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

fn resolve_round_stage<'a>(ctx: &RoundPlanningContext<'a, '_>) -> Option<&'a str> {
    ctx.activity_map
        .values()
        .find(|info| {
            info.activity_code == ctx.target.round_id
                || info
                    .activity_code
                    .starts_with(&format!("{}-g", ctx.target.round_id))
        })
        .and_then(|info| info.room_name)
}

fn plan_subsequent_round<'a>(ctx: &RoundPlanningContext<'a, '_>, plan: &mut ScorecardPlan<'a>) {
    let adv_result = AdvancementCalculator::calculate_blanks(ctx.comp, ctx.event, ctx.round);
    let round_stage = resolve_round_stage(ctx);
    let comp_name = ctx.comp.display_name();
    let time_limit_info =
        TimeLimitInfo::from_wcif(ctx.round.time_limit.as_ref(), ctx.round.cutoff.as_ref());

    if ctx.cover_sheets && adv_result.blank_count > 0 {
        let has_round = ctx.cover_sheets_by.contains(&CoverSheetBy::Round);
        let has_group = ctx.cover_sheets_by.contains(&CoverSheetBy::Group);
        let has_stage = ctx.cover_sheets_by.contains(&CoverSheetBy::Stage);

        // Tier 1 (Highest): Round cover sheet
        if has_round {
            plan.items.push(ScorecardItem::cover_sheet(
                comp_name,
                &ctx.event.id,
                ctx.event_name,
                ctx.target.round_number,
                0,
                None,
                ctx.attempt_count,
                adv_result.blank_count,
            ));
        }

        // Tier 2: Group cover sheet
        if has_group {
            plan.items.push(ScorecardItem::cover_sheet(
                comp_name,
                &ctx.event.id,
                ctx.event_name,
                ctx.target.round_number,
                1,
                None,
                ctx.attempt_count,
                adv_result.blank_count,
            ));
        }

        // Tier 3: Stage cover sheet (per group on each stage)
        if has_stage && (round_stage.is_some() || !has_group) {
            plan.items.push(ScorecardItem::cover_sheet(
                comp_name,
                &ctx.event.id,
                ctx.event_name,
                ctx.target.round_number,
                1,
                round_stage,
                ctx.attempt_count,
                adv_result.blank_count,
            ));
        }
    }

    for _ in 0..adv_result.blank_count {
        plan.items.push(ScorecardItem::blank(
            comp_name,
            &ctx.event.id,
            ctx.event_name,
            ctx.target.round_number,
            1,
            round_stage,
            ctx.attempt_count,
            time_limit_info,
        ));
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
        let activity_map = comp.build_activity_schedule_map();
        let mut notes = Vec::new();

        if requested_events.is_empty() {
            let mut targets = Vec::new();
            for event in &comp.events {
                if event.id == "333fm" {
                    continue;
                }
                for (round_idx, round) in event.rounds.iter().enumerate() {
                    let round_num = round_idx + 1;
                    let has_assignments =
                        has_competitor_assignments(comp, &activity_map, &round.id);
                    if round_num == 1 || has_assignments {
                        targets.push(GenerationTarget {
                            event_id: event.id.clone(),
                            round_id: round.id.clone(),
                            round_number: round_num,
                            is_open_round: round_num == 1 || has_assignments,
                        });
                    }
                }
            }
            return (targets, notes);
        }

        let mut targets = Vec::new();
        for arg in requested_events {
            let (event_id, round_num) = parse_event_arg(arg);
            if event_id == "333fm" {
                notes.push(
                    "Skipping '333fm' (3x3x3 Fewest Moves) as it does not use scorecards."
                        .to_string(),
                );
                continue;
            }

            let Some(event) = comp.events.iter().find(|e| e.id == event_id) else {
                continue;
            };

            match round_num {
                Some(r_num) => {
                    let round_id = format!("{}-r{}", event_id, r_num);
                    let has_assignments =
                        has_competitor_assignments(comp, &activity_map, &round_id);
                    targets.push(GenerationTarget {
                        event_id: event_id.clone(),
                        round_id,
                        round_number: r_num,
                        is_open_round: r_num == 1 || has_assignments,
                    });
                }
                None => {
                    for (round_idx, round) in event.rounds.iter().enumerate() {
                        let r_num = round_idx + 1;
                        let has_assignments =
                            has_competitor_assignments(comp, &activity_map, &round.id);
                        if r_num == 1 || has_assignments {
                            targets.push(GenerationTarget {
                                event_id: event_id.clone(),
                                round_id: round.id.clone(),
                                round_number: r_num,
                                is_open_round: r_num == 1 || has_assignments,
                            });
                        }
                    }
                }
            }
        }

        (targets, notes)
    }

    /// Plans and builds all required scorecards for the competition given the requested events, cover sheet option, and print_stations flag.
    pub fn plan<'a>(
        comp: &'a Competition,
        requested_events: &[String],
        cover_sheets: bool,
        cover_sheets_by: &[CoverSheetBy],
        print_stations: bool,
    ) -> Result<ScorecardPlan<'a>, Box<dyn Error>> {
        let (targets, notes) = Self::resolve_targets(comp, requested_events);
        let activity_map = comp.build_activity_schedule_map();
        let mut plan = ScorecardPlan::new(notes);

        for target in &targets {
            Self::plan_target_round(
                comp,
                target,
                &activity_map,
                cover_sheets,
                cover_sheets_by,
                print_stations,
                &mut plan,
            )?;
        }

        plan.assign_scorecard_numbers();
        Ok(plan)
    }

    fn plan_target_round<'a>(
        comp: &'a Competition,
        target: &GenerationTarget,
        activity_map: &FxHashMap<usize, ScheduledActivityInfo<'a>>,
        cover_sheets: bool,
        cover_sheets_by: &[CoverSheetBy],
        print_stations: bool,
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
            cover_sheets_by,
            print_stations,
        };

        if target.is_open_round {
            plan_open_round(&ctx, plan);
        } else {
            plan_subsequent_round(&ctx, plan);
        }
        Ok(())
    }
}
