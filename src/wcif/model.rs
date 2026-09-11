use std::fmt::{self, Display, Formatter};
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::{iter, str};

use rustc_hash::FxHashMap;
use serde::de::{Error as DeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 10-character ASCII identifier for a WCA competitor in the format `NNNNLLLLNN` (e.g. `"2022SMIT01"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WcaId([u8; 10]);

impl WcaId {
    /// Creates a `WcaId` from a 10-byte ASCII array. Returns `None` if not valid ASCII.
    #[must_use]
    pub const fn new(bytes: [u8; 10]) -> Option<Self> {
        if bytes.is_ascii() {
            Some(Self(bytes))
        } else {
            None
        }
    }

    /// Parses a 10-character ASCII WCA ID (e.g. `"2022SMIT01"`). Returns `None` if invalid.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let arr: [u8; 10] = s.as_bytes().try_into().ok()?;
        Self::new(arr)
    }

    /// Returns the WCA ID as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        str::from_utf8(&self.0).expect("ASCII bytes are valid UTF-8")
    }
}

impl Deref for WcaId {
    type Target = str;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Display for WcaId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<str> for WcaId {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for WcaId {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl Serialize for WcaId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

struct WcaIdVisitor;

impl Visitor<'_> for WcaIdVisitor {
    type Value = WcaId;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("a 10-character ASCII WCA ID")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        WcaId::parse(v.trim()).ok_or_else(|| DeError::custom("invalid WCA ID format"))
    }
}

impl<'de> Deserialize<'de> for WcaId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(WcaIdVisitor)
    }
}

struct OptionalWcaIdVisitor;

impl<'de> Visitor<'de> for OptionalWcaIdVisitor {
    type Value = Option<WcaId>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("null, empty string, or a 10-character ASCII WCA ID")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(None)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            WcaId::parse(trimmed)
                .map(Some)
                .ok_or_else(|| DeError::custom("invalid WCA ID format"))
        }
    }
}

/// Helper deserializer for WCIF `wcaId` which can be `null`, empty string `""`, or a 10-char ID.
pub fn deserialize_optional_wca_id<'de, D>(deserializer: D) -> Result<Option<WcaId>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_option(OptionalWcaIdVisitor)
}

/// 2-character ASCII country code (ISO 3166-1 alpha-2, e.g. `"US"`, `"CA"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct CountryIso2([u8; 2]);

impl CountryIso2 {
    /// Creates a `CountryIso2` from a 2-byte ASCII array. Returns `None` if not valid ASCII.
    #[must_use]
    pub const fn new(bytes: [u8; 2]) -> Option<Self> {
        if bytes.is_ascii() {
            Some(Self(bytes))
        } else {
            None
        }
    }

    /// Parses a 2-character ASCII country code (e.g. `"US"`). Returns `None` if invalid.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let arr: [u8; 2] = s.as_bytes().try_into().ok()?;
        Self::new(arr)
    }

    /// Returns the country code as a 2-byte ASCII array.
    #[inline]
    pub const fn to_bytes(self) -> [u8; 2] {
        self.0
    }

    /// Returns the country code as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        str::from_utf8(&self.0).expect("ASCII bytes are valid UTF-8")
    }
}

impl Deref for CountryIso2 {
    type Target = str;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Display for CountryIso2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<str> for CountryIso2 {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for CountryIso2 {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl From<CountryIso2> for [u8; 2] {
    fn from(code: CountryIso2) -> Self {
        code.0
    }
}

impl Serialize for CountryIso2 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

struct CountryIso2Visitor;

impl Visitor<'_> for CountryIso2Visitor {
    type Value = CountryIso2;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("a 2-character ASCII country code")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        CountryIso2::parse(v.trim()).ok_or_else(|| DeError::custom("invalid Country ISO2 format"))
    }
}

impl<'de> Deserialize<'de> for CountryIso2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(CountryIso2Visitor)
    }
}

struct OptionalCountryIso2Visitor;

impl<'de> Visitor<'de> for OptionalCountryIso2Visitor {
    type Value = Option<CountryIso2>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("null, empty string, or a 2-character ASCII country code")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(None)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            CountryIso2::parse(trimmed)
                .map(Some)
                .ok_or_else(|| DeError::custom("invalid Country ISO2 format"))
        }
    }
}

/// Helper deserializer for WCIF `countryIso2` which can be `null`, empty string `""`, or a 2-char code.
pub fn deserialize_optional_country_iso2<'de, D>(
    deserializer: D,
) -> Result<Option<CountryIso2>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_option(OptionalCountryIso2Visitor)
}

/// Competitor's personal best record for an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalBest {
    pub event_id: String,
    #[serde(rename = "type")]
    pub best_type: String,
    #[serde(default)]
    pub best: Option<i32>,
    #[serde(default)]
    pub world_ranking: Option<usize>,
    #[serde(default)]
    pub continental_ranking: Option<usize>,
    #[serde(default)]
    pub national_ranking: Option<usize>,
}

/// Represents a competitor or staff member.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub registrant_id: Option<NonZeroUsize>,
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_optional_wca_id")]
    pub wca_id: Option<WcaId>,
    #[serde(default, deserialize_with = "deserialize_optional_country_iso2")]
    pub country_iso2: Option<CountryIso2>,
    pub registration: Option<Registration>,
    #[serde(default)]
    pub assignments: Vec<Assignment>,
    #[serde(default)]
    pub personal_bests: Vec<PersonalBest>,
}

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

/// Scheduled activity info linking an activity code to its enclosing room/stage name.
#[derive(Debug, Clone, Copy)]
pub struct ScheduledActivityInfo<'a> {
    pub activity_code: &'a str,
    pub room_name: Option<&'a str>,
}

impl Competition {
    /// Deserializes a Competition struct from raw JSON bytes.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Returns the display name for the competition (`short_name` if available, else name).
    pub fn display_name(&self) -> &str {
        self.short_name.as_deref().unwrap_or(&self.name)
    }

    /// Builds a map of activity ID -> `ScheduledActivityInfo` (`activity_code`, `room_name`).
    ///
    /// NOTE: This traverses parent activities and their direct children (2 levels).
    /// WCA WCIF nesting is typically: Round Activity -> Group Activity, so this covers
    /// standard competition structures. Deeper nesting would require recursive traversal.
    pub fn build_activity_schedule_map(&self) -> FxHashMap<usize, ScheduledActivityInfo<'_>> {
        self.schedule
            .as_ref()
            .into_iter()
            .flat_map(Schedule::rooms)
            .flat_map(Room::activity_entries)
            .collect()
    }

    /// Finds and parses the Groupifier extension if present.
    pub fn get_groupifier_config(&self) -> Option<GroupifierCompetitionConfig> {
        self.extensions.iter().find_map(|ext| {
            if ext.id == "groupifier.CompetitionConfig" {
                GroupifierCompetitionConfig::deserialize(&ext.data).ok()
            } else if ext.id == "org.worldcubeassociation.groupifier" {
                GroupifierExtensionData::deserialize(&ext.data)
                    .ok()
                    .and_then(|ext_data| ext_data.competition_config)
            } else {
                None
            }
        })
    }

    /// Counts accepted competing competitors registered for a specific event.
    pub fn count_competitors_for_event(&self, event_id: &str) -> usize {
        self.accepted_competitors_for_event(event_id).count()
    }

    /// Returns an iterator over all accepted competitors registered for an event.
    pub fn accepted_competitors_for_event<'a>(
        &'a self,
        event_id: &str,
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

impl Person {
    /// Resolves the registrant ID either from `person.registrantId` or `person.registration.id`.
    pub fn registrant_id(&self) -> Option<NonZeroUsize> {
        self.registrant_id
            .or_else(|| self.registration.as_ref().and_then(|r| r.id))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registration {
    pub id: Option<NonZeroUsize>,
    pub status: Option<String>,
    #[serde(default)]
    pub event_ids: Vec<String>,
    #[serde(default)]
    pub is_competing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assignment {
    pub activity_id: usize,
    #[serde(rename = "assignmentCode")]
    pub code: Option<String>,
    pub station_number: Option<usize>,
}

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
    #[serde(default, alias = "scrambleSetCount")]
    pub scramble_group_count: usize,
}

impl Round {
    /// Determines the standard number of attempts for the round format.
    pub fn attempt_count(&self) -> usize {
        match self.format.as_deref() {
            Some("1") => 1,
            Some("2") => 2,
            Some("3" | "m") => 3,
            _ => 5,
        }
    }

    /// Returns true if this round is the final round of the given event in WCIF.
    pub fn is_final(&self, event: &Event) -> bool {
        event.rounds.last().is_some_and(|r| r.id == self.id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeLimit {
    pub centiseconds: i32,
    pub cumulative_round_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cutoff {
    pub number_of_attempts: usize,
    pub attempt_result: i32,
}

/// Helper deserializer for WCIF `AdvancementCondition` level which can be an integer (`16`) or float (`16.0`).
pub fn deserialize_optional_level<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrFloat {
        Int(usize),
        Float(f64),
    }

    let opt: Option<NumOrFloat> = Option::deserialize(deserializer)?;
    match opt {
        Some(NumOrFloat::Int(n)) => Ok(Some(n)),
        Some(NumOrFloat::Float(f)) => {
            if f >= 0.0 && f.is_finite() {
                Ok(Some(f.round() as usize))
            } else {
                Ok(None)
            }
        }
        None => Ok(None),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancementCondition {
    #[serde(rename = "type")]
    pub condition_type: String,
    #[serde(
        alias = "level",
        default,
        deserialize_with = "deserialize_optional_level"
    )]
    pub value: Option<usize>,
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

impl Schedule {
    /// Iterates over all rooms across all venues in the schedule.
    pub fn rooms(&self) -> impl Iterator<Item = &Room> {
        self.venues.iter().flat_map(|v| &v.rooms)
    }
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

impl Room {
    /// Returns an iterator over top-level and child activities paired with this room's name.
    pub fn activity_entries(&self) -> impl Iterator<Item = (usize, ScheduledActivityInfo<'_>)> {
        let room_name = self.name.as_deref();
        self.activities
            .iter()
            .flat_map(Activity::with_children)
            .map(move |a| {
                (
                    a.id,
                    ScheduledActivityInfo {
                        activity_code: &a.code,
                        room_name,
                    },
                )
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: usize,
    pub name: String,
    #[serde(rename = "activityCode")]
    pub code: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    #[serde(default)]
    pub child_activities: Vec<Activity>,
    #[serde(default)]
    pub scramble_set_id: Option<usize>,
}

impl Activity {
    /// Iterates over this activity and all of its immediate child activities.
    pub fn with_children(&self) -> impl Iterator<Item = &Activity> {
        iter::once(self).chain(&self.child_activities)
    }
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
    pub start_group_on_new_page: Option<bool>,
}
