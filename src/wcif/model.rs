use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// Root WCIF structure representing a WCA Competition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Competition {
    pub format_version: Option<String>,
    pub id: String,
    pub name: String,
    pub short_name: Option<String>,
    #[serde(default)]
    pub persons: Vec<Person>,
    #[serde(default)]
    pub events: Vec<Event>,
    pub schedule: Option<Schedule>,
    #[serde(default)]
    pub extensions: Vec<Extension>,
}

impl Competition {
    /// Returns the display name for the competition (short_name if available, else name).
    pub fn display_name(&self) -> &str {
        self.short_name.as_deref().unwrap_or(&self.name)
    }

    /// Builds a map of activity ID -> activityCode (e.g. 101 -> "333-r1-g1") from the schedule.
    pub fn build_activity_map(&self) -> FxHashMap<usize, &str> {
        self.schedule
            .as_ref()
            .into_iter()
            .flat_map(|s| &s.venues)
            .flat_map(|v| &v.rooms)
            .flat_map(|r| &r.activities)
            .flat_map(|a| std::iter::once(a).chain(&a.child_activities))
            .map(|a| (a.id, a.activity_code.as_str()))
            .collect()
    }

    /// Finds and parses the Groupifier extension if present.
    pub fn get_groupifier_config(&self) -> Option<GroupifierCompetitionConfig> {
        for ext in &self.extensions {
            if ext.id == "groupifier.CompetitionConfig"
                && let Ok(cfg) =
                    serde_json::from_value::<GroupifierCompetitionConfig>(ext.data.clone())
            {
                return Some(cfg);
            }
            if ext.id == "org.worldcubeassociation.groupifier"
                && let Ok(ext_data) =
                    serde_json::from_value::<GroupifierExtensionData>(ext.data.clone())
                && let Some(cfg) = ext_data.competition_config
            {
                return Some(cfg);
            }
        }
        None
    }

    /// Counts accepted competing competitors registered for a specific event.
    pub fn count_competitors_for_event(&self, event_id: &str) -> usize {
        self.accepted_competitors_for_event(event_id).count()
    }

    /// Returns an iterator over all accepted competitors registered for an event.
    pub fn accepted_competitors_for_event<'a>(
        &'a self,
        event_id: &'a str,
    ) -> impl Iterator<Item = &'a Person> {
        self.persons.iter().filter(move |person| {
            person.registration.as_ref().is_some_and(|reg| {
                reg.status.as_deref() == Some("accepted")
                    && reg.is_competing
                    && reg.event_ids.iter().any(|id| id == event_id)
            })
        })
    }
}

/// Represents a competitor or staff member.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub registrant_id: Option<usize>,
    pub name: String,
    pub wca_id: Option<String>,
    pub country_iso2: Option<String>,
    pub gender: Option<String>,
    pub registration: Option<Registration>,
    pub avatar: Option<Avatar>,
    pub roles: Option<Vec<String>>,
    #[serde(default)]
    pub assignments: Vec<Assignment>,
    #[serde(default)]
    pub personal_bests: Vec<PersonalBest>,
}

impl Person {
    /// Resolves the registrant ID either from `person.registrantId` or `person.registration.id`.
    pub fn registrant_id(&self) -> Option<usize> {
        self.registrant_id
            .or_else(|| self.registration.as_ref().and_then(|r| r.id))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registration {
    pub id: Option<usize>,
    pub status: Option<String>,
    #[serde(default)]
    pub event_ids: Vec<String>,
    #[serde(default)]
    pub is_competing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Avatar {
    pub url: Option<String>,
    pub thumb_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assignment {
    pub activity_id: usize,
    pub assignment_code: Option<String>,
    pub station_number: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalBest {
    pub event_id: String,
    #[serde(rename = "type")]
    pub pb_type: String,
    pub best: isize,
    pub world_ranking: Option<usize>,
    pub continental_ranking: Option<usize>,
    pub national_ranking: Option<usize>,
}

/// Represents an event holding multiple rounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    #[serde(default)]
    pub rounds: Vec<Round>,
    pub competitor_limit: Option<usize>,
    pub qualification: Option<serde_json::Value>,
}

/// Represents a specific round of an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Round {
    pub id: String,
    pub format: Option<String>,
    pub time_limit: Option<TimeLimit>,
    pub cutoff: Option<Cutoff>,
    pub advancement_condition: Option<AdvancementCondition>,
    #[serde(default)]
    pub scramble_group_count: usize,
}

impl Round {
    /// Determines the standard number of attempts for the round format.
    pub fn attempt_count(&self) -> usize {
        match self.format.as_deref() {
            Some("1") => 1,
            Some("2") => 2,
            Some("3") | Some("m") => 3,
            Some("5") | Some("a") => 5,
            _ => 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeLimit {
    pub centiseconds: isize,
    pub cumulative_round_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cutoff {
    pub number_of_attempts: usize,
    pub attempt_result: isize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancementCondition {
    #[serde(rename = "type")]
    pub condition_type: String,
    pub value: Option<f64>,
}

/// Competition schedule containing venues and rooms.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub start_date: Option<String>,
    pub number_of_days: Option<usize>,
    #[serde(default)]
    pub venues: Vec<Venue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Venue {
    pub id: Option<usize>,
    pub name: Option<String>,
    #[serde(default)]
    pub rooms: Vec<Room>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Room {
    pub id: Option<usize>,
    pub name: Option<String>,
    pub color: Option<String>,
    #[serde(default)]
    pub activities: Vec<Activity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: usize,
    pub name: String,
    pub activity_code: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    #[serde(default)]
    pub child_activities: Vec<Activity>,
    #[serde(default)]
    pub scramble_set_id: Option<usize>,
}

/// Generic WCIF extension container.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    pub id: String,
    pub spec_url: Option<String>,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupifierExtensionData {
    pub competition_config: Option<GroupifierCompetitionConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupifierCompetitionConfig {
    pub scorecard_paper_size: Option<String>,
    pub print_scorecards_cover_sheets: Option<bool>,
    pub scorecard_order: Option<String>,
    pub print_stations: Option<bool>,
    pub local_names_first: Option<bool>,
    pub print_one_name: Option<bool>,
    pub print_scramble_checker_for_top_ranked_competitors: Option<bool>,
    pub print_scramble_checker_for_final_rounds: Option<bool>,
    pub print_scramble_checker_for_blank_scorecards: Option<bool>,
}
