use super::events::{ActivityCode, WcaEvent};
use super::model::{PlannedRoundSummary, ScorecardItem, ScorecardPlan, TimeLimitInfo};
use crate::options::CoverSheetBy;
use crate::wcif::{
    AdvancementCalculator, Competition, Event, Person, Round, ScheduledActivityInfo,
};
use rustc_hash::FxHashMap;
use std::collections::BTreeMap;

/// Generation target representing an event and round to produce scorecards for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationTarget {
    pub event: WcaEvent,
    pub round_id: String,
    pub round_number: usize,
    pub is_open_round: bool,
}

impl GenerationTarget {
    /// Returns the corresponding round activity code.
    #[must_use]
    pub const fn activity_code(&self) -> ActivityCode {
        ActivityCode::round(self.event, self.round_number)
    }
}

/// Parsed representation of an event CLI argument (e.g. "333", "333-r1", "333-1").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedEventArg {
    pub event: WcaEvent,
    pub round_number: Option<usize>,
}

fn parse_event_arg(arg: &str) -> Option<ParsedEventArg> {
    if let Some((event_str, round_str)) = arg.split_once('-') {
        let event = WcaEvent::from_id(event_str)?;
        let round_number = round_str.trim_start_matches('r').parse::<usize>().ok();
        Some(ParsedEventArg {
            event,
            round_number,
        })
    } else {
        let event = WcaEvent::from_id(arg)?;
        Some(ParsedEventArg {
            event,
            round_number: None,
        })
    }
}

#[inline]
fn matches_round_activity(activity_code: &str, round_target: ActivityCode) -> bool {
    if let Some(code) = ActivityCode::parse(activity_code) {
        code.matches_round(round_target.event, round_target.round_number)
    } else {
        let round_id = round_target.round_id();
        activity_code
            .strip_prefix(&round_id)
            .is_some_and(|rem| rem.is_empty() || rem.starts_with("-g"))
    }
}

#[inline]
fn extract_group_number(activity_code: &str, round_target: ActivityCode) -> Option<usize> {
    if let Some(code) = ActivityCode::parse(activity_code) {
        if code.matches_round(round_target.event, round_target.round_number) {
            Some(code.group_or_default())
        } else {
            None
        }
    } else {
        let round_id = round_target.round_id();
        let rem = activity_code.strip_prefix(&round_id)?;
        if let Some(stripped) = rem.strip_prefix("-g") {
            Some(stripped.parse().unwrap_or(1))
        } else if rem.is_empty() {
            Some(1)
        } else {
            None
        }
    }
}

fn has_competitor_assignments(
    comp: &Competition,
    activity_map: &FxHashMap<usize, ScheduledActivityInfo<'_>>,
    round_target: ActivityCode,
) -> bool {
    comp.persons
        .iter()
        .flat_map(|p| &p.assignments)
        .any(|assign| {
            (assign.code.as_deref() == Some("competitor") || assign.code.is_none())
                && activity_map
                    .get(&assign.activity_id)
                    .is_some_and(|info| matches_round_activity(info.activity_code, round_target))
        })
}

/// Error encountered during scorecard planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannerError {
    EventNotFound(String),
    RoundNotFound { event_id: String, round_id: String },
    UnsupportedEvent(String),
}

impl std::fmt::Display for PlannerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlannerError::EventNotFound(id) => write!(f, "Event '{id}' not found in WCIF"),
            PlannerError::RoundNotFound { event_id, round_id } => {
                write!(
                    f,
                    "Round '{round_id}' for event '{event_id}' not found in WCIF schedule"
                )
            }
            PlannerError::UnsupportedEvent(id) => {
                write!(f, "Unknown or unsupported WCA event ID: '{id}'")
            }
        }
    }
}

impl std::error::Error for PlannerError {}

fn find_event_and_round<'a>(
    comp: &'a Competition,
    target: &GenerationTarget,
) -> Result<(&'a Event, &'a Round), PlannerError> {
    let event = comp
        .events
        .iter()
        .find(|e| e.id == target.event.code())
        .ok_or_else(|| PlannerError::EventNotFound(target.event.code().to_string()))?;

    let round = event
        .rounds
        .iter()
        .enumerate()
        .find(|(idx, r)| r.id == target.round_id || idx + 1 == target.round_number)
        .map(|(_, r)| r)
        .ok_or_else(|| PlannerError::RoundNotFound {
            event_id: target.event.code().to_string(),
            round_id: target.round_id.clone(),
        })?;

    Ok((event, round))
}

fn resolve_assignment<'a>(
    person: &'a Person,
    activity_map: &FxHashMap<usize, ScheduledActivityInfo<'a>>,
    round_target: ActivityCode,
) -> Option<(usize, Option<usize>, Option<&'a str>)> {
    person.assignments.iter().find_map(|assign| {
        let is_competitor = assign.code.as_deref() == Some("competitor") || assign.code.is_none();
        if !is_competitor {
            return None;
        }
        let info = activity_map.get(&assign.activity_id)?;
        let group_num = extract_group_number(info.activity_code, round_target)?;
        Some((group_num, assign.station_number, info.room_name))
    })
}

/// Formats a competitor's name. When `print_one_name` is true, removes any parenthesized local or Latin name.
/// Currently this assumes that all names conform to the WCA spec, if someone submits an invalid name (i.e. with
/// non-ascii parens or in the incorrect format, this could mangle the name in unintended ways.
pub fn format_competitor_name(name: &str, print_one_name: bool) -> &str {
    if !print_one_name {
        return name;
    }
    if let Some(idx) = name.find('(') {
        name[..idx].trim_end()
    } else {
        name
    }
}

struct RoundPlanningContext<'a, 'b> {
    comp: &'a Competition,
    target: &'b GenerationTarget,
    event: &'a Event,
    round: &'a Round,
    attempt_count: usize,
    activity_map: &'b FxHashMap<usize, ScheduledActivityInfo<'a>>,
    cover_sheets: bool,
    cover_sheets_by: &'b [CoverSheetBy],
    print_stations: bool,
    print_one_name: bool,
}

type GroupKey<'a> = (usize, Option<&'a str>);
type GroupMap<'a> = BTreeMap<GroupKey<'a>, Vec<ScorecardItem<'a>>>;

fn collect_open_round_competitors<'a>(
    ctx: &RoundPlanningContext<'a, '_>,
) -> (GroupMap<'a>, usize, Vec<String>) {
    let comp_name = ctx.comp.display_name();
    let time_limit_info =
        TimeLimitInfo::from_wcif(ctx.round.time_limit.as_ref(), ctx.round.cutoff.as_ref());
    let competitors: Vec<_> = ctx
        .comp
        .accepted_competitors_for_event(&ctx.event.id)
        .filter_map(|person| {
            let assignment =
                resolve_assignment(person, ctx.activity_map, ctx.target.activity_code());

            let (group_num, station_num, stage_name) = match assignment {
                Some(a) => a,
                None => {
                    if ctx.target.round_number == 1 {
                        (1, None, None)
                    } else {
                        return None;
                    }
                }
            };

            let station_num = if ctx.print_stations {
                station_num
            } else {
                None
            };

            Some((person, group_num, station_num, stage_name))
        })
        .collect();

    let count = competitors.len();
    let sample_names: Vec<String> = competitors
        .iter()
        .take(5)
        .map(|(person, ..)| format_competitor_name(&person.name, ctx.print_one_name).to_string())
        .collect();

    let groups = competitors.into_iter().fold(
        GroupMap::new(),
        |mut acc, (person, group_num, station_num, stage_name)| {
            let item = ScorecardItem::competitor(
                comp_name,
                ctx.target.event,
                ctx.target.round_number,
                group_num,
                stage_name,
                format_competitor_name(person.name.as_str(), ctx.print_one_name),
                person.registrant_id(),
                person.wca_id.as_deref(),
                station_num,
                ctx.attempt_count,
                time_limit_info,
            );
            acc.entry((group_num, stage_name)).or_default().push(item);
            acc
        },
    );

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
    let mut group_totals: FxHashMap<usize, usize> = FxHashMap::default();
    for ((group_num, _), cards) in &groups {
        *group_totals.entry(*group_num).or_default() += cards.len();
    }

    // Tier 1 (Highest): Round cover sheet for entire round of the event
    if has_round && count > 0 {
        plan.items.push(ScorecardItem::cover_sheet(
            ctx.comp.display_name(),
            ctx.target.event,
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
                ctx.target.event,
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
                ctx.target.event,
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
        event: ctx.target.event,
        round_number: ctx.target.round_number,
        competitor_count: count,
        sample_competitor_names: sample_names,
    });
}

fn resolve_round_stage<'a>(ctx: &RoundPlanningContext<'a, '_>) -> Option<&'a str> {
    ctx.activity_map
        .values()
        .find(|info| matches_round_activity(info.activity_code, ctx.target.activity_code()))
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
                ctx.target.event,
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
                ctx.target.event,
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
                ctx.target.event,
                ctx.target.round_number,
                1,
                round_stage,
                ctx.attempt_count,
                adv_result.blank_count,
            ));
        }
    }

    plan.items.extend((0..adv_result.blank_count).map(|_| {
        ScorecardItem::blank(
            comp_name,
            ctx.target.event,
            ctx.target.round_number,
            1,
            round_stage,
            ctx.attempt_count,
            time_limit_info,
        )
    }));

    plan.summaries.push(PlannedRoundSummary::SubsequentRound {
        event: ctx.target.event,
        round_number: ctx.target.round_number,
        blank_count: adv_result.blank_count,
        reason: adv_result.reason,
    });
}

#[derive(Debug, Clone, Copy)]
struct PlanConfig<'a> {
    cover_sheets: bool,
    cover_sheets_by: &'a [CoverSheetBy],
    print_stations: bool,
    print_one_name: bool,
}

/// Planner responsible for extracting competitor assignments, calculating advancement blanks, and sequencing scorecards.
pub struct ScorecardPlanner;

impl ScorecardPlanner {
    /// Resolves all open round generation targets for the competition.
    pub fn resolve_all_targets(comp: &Competition) -> (Vec<GenerationTarget>, Vec<String>) {
        Self::resolve_targets::<&str>(comp, &[])
    }

    fn targets_for_event<'a>(
        comp: &'a Competition,
        activity_map: &'a FxHashMap<usize, ScheduledActivityInfo<'a>>,
        event: &'a Event,
    ) -> impl Iterator<Item = GenerationTarget> + 'a {
        let wca_event = WcaEvent::from_id(&event.id);
        event
            .rounds
            .iter()
            .enumerate()
            .filter_map(move |(idx, round)| {
                let wca_event = wca_event?;
                let round_number = idx + 1;
                let round_activity = ActivityCode::round(wca_event, round_number);
                let has_assignments =
                    has_competitor_assignments(comp, activity_map, round_activity);
                (round_number == 1 || has_assignments).then(|| GenerationTarget {
                    event: wca_event,
                    round_id: round.id.clone(),
                    round_number,
                    is_open_round: round_number == 1 || has_assignments,
                })
            })
    }

    /// Resolves generation targets from CLI arguments (or all open rounds if none specified), returning targets and any diagnostic notes.
    /// Omits events that do not use scorecards (such as 3x3x3 Fewest Moves - 333fm).
    pub fn resolve_targets<S: AsRef<str>>(
        comp: &Competition,
        requested_events: &[S],
    ) -> (Vec<GenerationTarget>, Vec<String>) {
        let activity_map = comp.build_activity_schedule_map();
        let mut notes = Vec::new();

        if requested_events.is_empty() {
            let targets = comp
                .events
                .iter()
                .filter(|e| e.id != "333fm")
                .flat_map(|event| Self::targets_for_event(comp, &activity_map, event))
                .collect();
            return (targets, notes);
        }

        let mut targets = Vec::new();
        for arg in requested_events {
            let Some(parsed) = parse_event_arg(arg.as_ref()) else {
                continue;
            };
            if parsed.event == WcaEvent::E333Fm {
                notes.push(
                    "Skipping '333fm' (3x3x3 Fewest Moves) as it does not use scorecards."
                        .to_string(),
                );
                continue;
            }

            let Some(event) = comp.events.iter().find(|e| e.id == parsed.event.code()) else {
                continue;
            };

            if let Some(r_num) = parsed.round_number {
                let round_activity = ActivityCode::round(parsed.event, r_num);
                let has_assignments =
                    has_competitor_assignments(comp, &activity_map, round_activity);
                targets.push(GenerationTarget {
                    event: parsed.event,
                    round_id: round_activity.round_id(),
                    round_number: r_num,
                    is_open_round: r_num == 1 || has_assignments,
                });
            } else {
                targets.extend(Self::targets_for_event(comp, &activity_map, event));
            }
        }

        (targets, notes)
    }

    /// Plans and builds all required scorecards for the competition given the requested events, cover sheet option, `print_stations`, and `print_one_name` flag.
    pub fn plan<'a, S: AsRef<str>>(
        comp: &'a Competition,
        requested_events: &[S],
        cover_sheets: bool,
        cover_sheets_by: &[CoverSheetBy],
        print_stations: bool,
        print_one_name: bool,
    ) -> Result<ScorecardPlan<'a>, PlannerError> {
        let (targets, notes) = Self::resolve_targets(comp, requested_events);
        let activity_map = comp.build_activity_schedule_map();
        let mut plan = ScorecardPlan::new(notes);
        let config = PlanConfig {
            cover_sheets,
            cover_sheets_by,
            print_stations,
            print_one_name,
        };

        for target in &targets {
            Self::plan_target_round(comp, target, &activity_map, config, &mut plan)?;
        }

        plan.assign_scorecard_numbers();
        Ok(plan)
    }

    fn plan_target_round<'a>(
        comp: &'a Competition,
        target: &GenerationTarget,
        activity_map: &FxHashMap<usize, ScheduledActivityInfo<'a>>,
        config: PlanConfig<'_>,
        plan: &mut ScorecardPlan<'a>,
    ) -> Result<(), PlannerError> {
        let (event, round) = find_event_and_round(comp, target)?;
        let attempt_count = round.attempt_count();

        let ctx = RoundPlanningContext {
            comp,
            target,
            event,
            round,
            attempt_count,
            activity_map,
            cover_sheets: config.cover_sheets,
            cover_sheets_by: config.cover_sheets_by,
            print_stations: config.print_stations,
            print_one_name: config.print_one_name,
        };

        if target.is_open_round {
            plan_open_round(&ctx, plan);
        } else {
            plan_subsequent_round(&ctx, plan);
        }
        Ok(())
    }
}
