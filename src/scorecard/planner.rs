use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use rustc_hash::FxHashMap;

use super::events::{ActivityCode, RoundId, WcaEvent};
use super::model::{
    BlankScorecard, Competitor, CoverSheet, GroupNumber, PlannedRoundSummary, RoundNumber,
    Scorecard, ScorecardItem, ScorecardPlan, TimeLimitInfo,
};
use crate::options::CoverSheetBy;
use crate::wcif::{
    AdvancementCalculator, Competition, Event, Person, Round, ScheduledActivityInfo,
};

/// Generation target representing an event and round to produce scorecards for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationTarget {
    pub round_id: RoundId,
    pub is_open_round: bool,
}

impl GenerationTarget {
    /// Returns the corresponding round activity code.
    #[must_use]
    pub const fn activity_code(&self) -> ActivityCode {
        ActivityCode::from_round(self.round_id)
    }
}

/// Parsed representation of an event CLI argument (e.g. "333", "333-r1", "333-1").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedEventArg {
    pub event: WcaEvent,
    pub round_number: Option<RoundNumber>,
}

fn parse_event_arg(arg: &str) -> Option<ParsedEventArg> {
    if let Some((event_str, round_str)) = arg.split_once('-') {
        let event = WcaEvent::from_id(event_str)?;
        let round_number = round_str
            .trim_start_matches('r')
            .parse::<RoundNumber>()
            .ok();
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
        activity_code
            .strip_prefix(round_target.event.code())
            .and_then(|rem| rem.strip_prefix("-r"))
            .and_then(|rem| {
                let (r_str, group_rem) = rem.split_once("-g").unwrap_or((rem, ""));
                (r_str.parse::<RoundNumber>().ok() == Some(round_target.round_number))
                    .then_some(group_rem)
            })
            .is_some_and(|rem| rem.is_empty() || rem.starts_with("-g"))
    }
}

#[inline]
fn extract_group_number(activity_code: &str, round_target: ActivityCode) -> Option<GroupNumber> {
    if let Some(code) = ActivityCode::parse(activity_code) {
        code.matches_round(round_target.event, round_target.round_number)
            .then(|| code.group_or_default())
    } else {
        let event_rem = activity_code.strip_prefix(round_target.event.code())?;
        let rem = event_rem.strip_prefix("-r")?;
        let (r_str, group_rem) = rem.split_once("-g").unwrap_or((rem, ""));
        if r_str.parse::<RoundNumber>().ok() != Some(round_target.round_number) {
            return None;
        }
        if let Some(stripped) = group_rem.strip_prefix("-g") {
            Some(stripped.parse().unwrap_or(1))
        } else if group_rem.is_empty() {
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
    RoundNotFound(RoundId),
    UnsupportedEvent(String),
}

impl Display for PlannerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PlannerError::EventNotFound(id) => write!(f, "Event '{id}' not found in WCIF"),
            PlannerError::RoundNotFound(round_id) => {
                write!(
                    f,
                    "Round '{round_id}' for event '{}' not found in WCIF schedule",
                    round_id.event.code()
                )
            }
            PlannerError::UnsupportedEvent(id) => {
                write!(f, "Unknown or unsupported WCA event ID: '{id}'")
            }
        }
    }
}

impl Error for PlannerError {}

fn find_event_and_round<'a>(
    comp: &'a Competition,
    target: &GenerationTarget,
) -> Result<(&'a Event, &'a Round), PlannerError> {
    let event = comp
        .events
        .iter()
        .find(|e| e.id == target.round_id.event.code())
        .ok_or_else(|| PlannerError::EventNotFound(target.round_id.event.code().to_owned()))?;

    let round = event
        .rounds
        .iter()
        .enumerate()
        .find(|(idx, r)| {
            u32::try_from(idx + 1).ok() == Some(target.round_id.round_number)
                || RoundId::parse(&r.id) == Some(target.round_id)
        })
        .map(|(_, r)| r)
        .ok_or(PlannerError::RoundNotFound(target.round_id))?;

    Ok((event, round))
}

fn resolve_assignment<'a>(
    person: &'a Person,
    activity_map: &FxHashMap<usize, ScheduledActivityInfo<'a>>,
    round_target: ActivityCode,
) -> Option<(GroupNumber, Option<usize>, Option<&'a str>)> {
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

/// Splits a competitor's name into primary name and optional parenthesized local name.
/// E.g. `"Marco Yang (杨柯辰)"` -> `("Marco Yang", Some("杨柯辰"))`.
pub fn parse_competitor_name(name: &str) -> (&str, Option<&str>) {
    if let Some(open) = name.find('(')
        && let Some(close) = name[open..].find(')')
    {
        let primary = name[..open].trim();
        let local = name[open + 1..open + close].trim();
        if !local.is_empty() {
            return (primary, Some(local));
        }
    }
    (name.trim(), None)
}

/// Formats a competitor's name as a `(primary, Option<local>)` pair.
/// When `print_one_name` is true, the local name is omitted (`None`).
pub fn format_competitor_name(
    name: &str,
    print_one_name: bool,
    local_names_first: bool,
) -> (&str, Option<&str>) {
    let (primary, local) = parse_competitor_name(name);
    if print_one_name {
        return (primary, None);
    }

    if let Some(loc) = local
        && local_names_first
    {
        return (loc, Some(primary));
    }

    (primary, local)
}

pub const SCRAMBLE_CHECKER_EXCLUDED_EVENTS: &[&str] = &["555", "666", "777", "minx"];

/// Determines if a competitor scorecard should print a scramble checker box.
pub fn should_print_scramble_checker_for_competitor(
    person: &Person,
    event: &Event,
    round: &Round,
    config: &PlanConfig<'_>,
) -> bool {
    if SCRAMBLE_CHECKER_EXCLUDED_EVENTS.contains(&event.id.as_str()) {
        return false;
    }
    if config.scramble_checker_top_ranked {
        let single_qualifies = person
            .personal_bests
            .iter()
            .find(|pb| pb.event_id == event.id && pb.best_type == "single")
            .and_then(|pb| pb.world_ranking)
            .is_some_and(|wr| wr <= 50 && wr > 0);

        let avg_qualifies = person
            .personal_bests
            .iter()
            .find(|pb| pb.event_id == event.id && pb.best_type == "average")
            .is_some_and(|pb| {
                pb.world_ranking.is_some_and(|wr| wr <= 50 && wr > 0)
                    || pb.national_ranking.is_some_and(|nr| nr <= 15 && nr > 0)
            });

        if single_qualifies || avg_qualifies {
            return true;
        }
    }
    if config.scramble_checker_final_rounds && round.is_final(event) {
        return true;
    }
    false
}

/// Determines if a blank scorecard should print a scramble checker box.
pub fn should_print_scramble_checker_for_blank(
    event: &Event,
    round: &Round,
    config: &PlanConfig<'_>,
) -> bool {
    if SCRAMBLE_CHECKER_EXCLUDED_EVENTS.contains(&event.id.as_str()) {
        return false;
    }
    if config.scramble_checker_blank {
        return true;
    }
    if config.scramble_checker_final_rounds && round.is_final(event) {
        return true;
    }
    false
}

struct RoundPlanningContext<'a, 'b> {
    comp: &'a Competition,
    target: &'b GenerationTarget,
    event: &'a Event,
    round: &'a Round,
    attempt_count: usize,
    activity_map: &'b FxHashMap<usize, ScheduledActivityInfo<'a>>,
    config: &'b PlanConfig<'b>,
}

type GroupKey<'a> = (GroupNumber, Option<&'a str>);
type GroupMap<'a> = BTreeMap<GroupKey<'a>, Vec<ScorecardItem<'a>>>;

fn collect_open_round_competitors<'a>(
    ctx: &RoundPlanningContext<'a, '_>,
) -> (GroupMap<'a>, usize, Vec<String>) {
    let comp_name = ctx.comp.display_name();
    let time_limit_info =
        TimeLimitInfo::from_wcif(ctx.round.time_limit.as_ref(), ctx.round.cutoff.as_ref());
    let mut groups = GroupMap::new();
    let mut count = 0;
    let mut sample_names = Vec::with_capacity(5);

    for person in ctx.comp.accepted_competitors_for_event(&ctx.event.id) {
        let assignment = resolve_assignment(person, ctx.activity_map, ctx.target.activity_code());

        let (group_num, assigned_station, stage_name) = match assignment {
            Some(a) => a,
            None => {
                if ctx.target.round_id.round_number == 1 {
                    (1, None, None)
                } else {
                    continue;
                }
            }
        };

        let station_num = if ctx.config.print_stations {
            assigned_station
        } else {
            None
        };

        count += 1;
        if sample_names.len() < 5 {
            let (primary, local) = format_competitor_name(
                &person.name,
                ctx.config.print_one_name,
                ctx.config.local_names_first,
            );
            sample_names.push(match local {
                Some(loc) => format!("{primary} ({loc})"),
                None => primary.to_owned(),
            });
        }

        let needs_scramble_checker =
            should_print_scramble_checker_for_competitor(person, ctx.event, ctx.round, ctx.config);

        let competitor = Competitor::from_person(
            person,
            ctx.config.print_one_name,
            ctx.config.local_names_first,
        );
        let item = Scorecard::new(
            comp_name,
            ctx.target.round_id.event,
            ctx.target.round_id.round_number,
            group_num,
            competitor,
        )
        .with_stage(stage_name)
        .with_station(station_num)
        .with_attempts(ctx.attempt_count)
        .with_time_limit(time_limit_info)
        .with_scramble_checker(needs_scramble_checker)
        .into();
        groups
            .entry((group_num, stage_name))
            .or_default()
            .push(item);
    }

    (groups, count, sample_names)
}

/// Sorts cards within a group:
/// 1. Cards with station numbers come first, sorted by station number ascending.
/// 2. Cards without station numbers (or sharing the same station number) are sorted alphabetically by competitor.
pub fn sort_group_cards(cards: &mut [ScorecardItem<'_>]) {
    cards.sort_by(|a, b| match (a, b) {
        (ScorecardItem::Scorecard(sc_a), ScorecardItem::Scorecard(sc_b)) => {
            match (sc_a.station_number, sc_b.station_number) {
                (Some(s_a), Some(s_b)) => s_a
                    .cmp(&s_b)
                    .then_with(|| sc_a.competitor.cmp(&sc_b.competitor)),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => sc_a.competitor.cmp(&sc_b.competitor),
            }
        }
        _ => Ordering::Equal,
    });
}

fn plan_open_round<'a>(ctx: &RoundPlanningContext<'a, '_>, plan: &mut ScorecardPlan<'a>) {
    let (groups, count, sample_names) = collect_open_round_competitors(ctx);

    if count == 0 && ctx.target.round_id.round_number > 1 {
        plan_subsequent_round(ctx, plan);
        return;
    }

    let has_round =
        ctx.config.cover_sheets && ctx.config.cover_sheets_by.contains(&CoverSheetBy::Round);
    let has_group =
        ctx.config.cover_sheets && ctx.config.cover_sheets_by.contains(&CoverSheetBy::Group);
    let has_stage =
        ctx.config.cover_sheets && ctx.config.cover_sheets_by.contains(&CoverSheetBy::Stage);

    // Pre-calculate card counts per group (across all stages)
    let mut group_totals: FxHashMap<GroupNumber, usize> = FxHashMap::default();
    for ((group_num, _), cards) in &groups {
        *group_totals.entry(*group_num).or_default() += cards.len();
    }

    // Tier 1 (Highest): Round cover sheet for entire round of the event
    if has_round && count > 0 {
        plan.items.push(
            CoverSheet::new(
                ctx.comp.display_name(),
                ctx.target.round_id.event,
                ctx.target.round_id.round_number,
                0,
                count,
            )
            .into(),
        );
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
            plan.items.push(
                CoverSheet::new(
                    ctx.comp.display_name(),
                    ctx.target.round_id.event,
                    ctx.target.round_id.round_number,
                    group_num,
                    group_total,
                )
                .into(),
            );
            current_group = Some(group_num);
        }

        // Tier 3: Stage cover sheet (per group on each stage)
        // If stage is None and group cover sheet was already emitted, avoid duplicate cover sheet
        if has_stage && (stage_name.is_some() || !has_group) {
            plan.items.push(
                CoverSheet::new(
                    ctx.comp.display_name(),
                    ctx.target.round_id.event,
                    ctx.target.round_id.round_number,
                    group_num,
                    card_list.len(),
                )
                .with_stage(stage_name)
                .into(),
            );
        }

        plan.items.extend(card_list);
    }

    plan.summaries.push(PlannedRoundSummary::OpenRound {
        event: ctx.target.round_id.event,
        round_number: ctx.target.round_id.round_number,
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

    if ctx.config.cover_sheets && adv_result.blank_count > 0 {
        let has_round = ctx.config.cover_sheets_by.contains(&CoverSheetBy::Round);
        let has_group = ctx.config.cover_sheets_by.contains(&CoverSheetBy::Group);
        let has_stage = ctx.config.cover_sheets_by.contains(&CoverSheetBy::Stage);

        // Tier 1 (Highest): Round cover sheet
        if has_round {
            plan.items.push(
                CoverSheet::new(
                    comp_name,
                    ctx.target.round_id.event,
                    ctx.target.round_id.round_number,
                    0,
                    adv_result.blank_count,
                )
                .into(),
            );
        }

        // Tier 2: Group cover sheet
        if has_group {
            plan.items.push(
                CoverSheet::new(
                    comp_name,
                    ctx.target.round_id.event,
                    ctx.target.round_id.round_number,
                    1,
                    adv_result.blank_count,
                )
                .into(),
            );
        }

        // Tier 3: Stage cover sheet (per group on each stage)
        if has_stage && (round_stage.is_some() || !has_group) {
            plan.items.push(
                CoverSheet::new(
                    comp_name,
                    ctx.target.round_id.event,
                    ctx.target.round_id.round_number,
                    1,
                    adv_result.blank_count,
                )
                .with_stage(round_stage)
                .into(),
            );
        }
    }

    let needs_scramble_checker =
        should_print_scramble_checker_for_blank(ctx.event, ctx.round, ctx.config);

    plan.items.extend((0..adv_result.blank_count).map(|_| {
        ScorecardItem::Blank(
            BlankScorecard::new(
                comp_name,
                ctx.target.round_id.event,
                ctx.target.round_id.round_number,
                1,
            )
            .with_stage(round_stage)
            .with_attempts(ctx.attempt_count)
            .with_time_limit(time_limit_info)
            .with_scramble_checker(needs_scramble_checker),
        )
    }));

    plan.summaries.push(PlannedRoundSummary::SubsequentRound {
        event: ctx.target.round_id.event,
        round_number: ctx.target.round_id.round_number,
        blank_count: adv_result.blank_count,
        reason: adv_result.reason,
    });
}

/// Configuration options for planning scorecards.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlanConfig<'a> {
    pub cover_sheets: bool,
    pub cover_sheets_by: &'a [CoverSheetBy],
    pub print_stations: bool,
    pub print_one_name: bool,
    pub local_names_first: bool,
    pub scramble_checker_top_ranked: bool,
    pub scramble_checker_final_rounds: bool,
    pub scramble_checker_blank: bool,
}

#[cfg(test)]
impl<'a> PlanConfig<'a> {
    pub fn new(
        cover_sheets: bool,
        cover_sheets_by: &'a [CoverSheetBy],
        print_stations: bool,
        print_one_name: bool,
        local_names_first: bool,
    ) -> Self {
        Self {
            cover_sheets,
            cover_sheets_by,
            print_stations,
            print_one_name,
            local_names_first,
            scramble_checker_top_ranked: false,
            scramble_checker_final_rounds: false,
            scramble_checker_blank: false,
        }
    }

    pub fn with_scramble_checker(
        mut self,
        top_ranked: bool,
        final_rounds: bool,
        blank: bool,
    ) -> Self {
        self.scramble_checker_top_ranked = top_ranked;
        self.scramble_checker_final_rounds = final_rounds;
        self.scramble_checker_blank = blank;
        self
    }
}

impl<'a> From<&'a crate::options::ResolvedOptions> for PlanConfig<'a> {
    fn from(opts: &'a crate::options::ResolvedOptions) -> Self {
        Self {
            cover_sheets: opts.cover_sheets,
            cover_sheets_by: &opts.cover_sheets_by,
            print_stations: opts.print_stations,
            print_one_name: opts.print_one_name,
            local_names_first: opts.local_names_first,
            scramble_checker_top_ranked: opts.scramble_checker_top_ranked,
            scramble_checker_final_rounds: opts.scramble_checker_final_rounds,
            scramble_checker_blank: opts.scramble_checker_blank,
        }
    }
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
        let opt_wca_event = WcaEvent::from_id(&event.id);
        event
            .rounds
            .iter()
            .enumerate()
            .filter_map(move |(idx, _round)| {
                let wca_event = opt_wca_event?;
                let round_number = u32::try_from(idx + 1).ok()?;
                let round_id = RoundId::new(wca_event, round_number);
                let round_activity = ActivityCode::from_round(round_id);
                let has_assignments =
                    has_competitor_assignments(comp, activity_map, round_activity);
                (round_number == 1 || has_assignments).then_some(GenerationTarget {
                    round_id,
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
                        .to_owned(),
                );
                continue;
            }

            let Some(event) = comp.events.iter().find(|e| e.id == parsed.event.code()) else {
                continue;
            };

            if let Some(r_num) = parsed.round_number {
                let round_id = RoundId::new(parsed.event, r_num);
                let round_activity = ActivityCode::from_round(round_id);
                let has_assignments =
                    has_competitor_assignments(comp, &activity_map, round_activity);
                targets.push(GenerationTarget {
                    round_id,
                    is_open_round: r_num == 1 || has_assignments,
                });
            } else {
                targets.extend(Self::targets_for_event(comp, &activity_map, event));
            }
        }

        (targets, notes)
    }

    /// Plans and builds all required scorecards for the competition with the given configuration.
    pub fn plan<'a, S: AsRef<str>>(
        comp: &'a Competition,
        requested_events: &[S],
        config: PlanConfig<'_>,
    ) -> Result<ScorecardPlan<'a>, PlannerError> {
        let (targets, notes) = Self::resolve_targets(comp, requested_events);
        let activity_map = comp.build_activity_schedule_map();
        let mut plan = ScorecardPlan::new(notes);

        for target in &targets {
            Self::plan_target_round(comp, target, &activity_map, config, &mut plan)?;
        }

        plan.assign_numbers();
        Ok(plan)
    }

    /// Convenience wrapper to plan scorecards directly from [`ResolvedOptions`].
    pub fn plan_with_options<'a, S: AsRef<str>>(
        comp: &'a Competition,
        requested_events: &[S],
        opts: &crate::options::ResolvedOptions,
    ) -> Result<ScorecardPlan<'a>, PlannerError> {
        Self::plan(comp, requested_events, PlanConfig::from(opts))
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
            config: &config,
        };

        if target.is_open_round {
            plan_open_round(&ctx, plan);
        } else {
            plan_subsequent_round(&ctx, plan);
        }
        Ok(())
    }
}
