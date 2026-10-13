//! Config schema. Everything a ring draws is data; nothing here touches the clock.

use std::{fmt, path::PathBuf, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// Winter, spring, summer, autumn.
#[derive(Debug, Clone)]
pub struct Palette {
    pub background: String,
    pub foreground: String,
    pub muted: String,
    /// Where "now" is: hands, today markers.
    pub accent: String,
    /// Sunlight: solstices, equinoxes, sunrise and sunset. Never `accent`, so a
    /// solar mark is never mistaken for now.
    pub solar: String,
    pub weekend: String,
    /// Week ticks on the year ring. Red, like a weekend.
    pub week_marks: String,
    /// Month rules and anything else drawn *on* a light season colour.
    pub border: String,
    pub seasons: [String; 4],
}

impl Palette {
    pub fn resolve(theme: Theme, over: &Overrides) -> Self {
        let (background, foreground, muted, accent, solar, weekend, week_marks, border, seasons) =
            match theme {
                Theme::Dark => (
                    "#0e1116",
                    "#e8e6e1",
                    "#39404d",
                    "#ff9f1c",
                    "#4cc9f0",
                    "#c1121f",
                    "#e63946",
                    "#12100e",
                    ["#cfe0f3", "#63b862", "#e0c56e", "#c08552"],
                ),
                Theme::Light => (
                    "#f4f1ea",
                    "#1b1b1b",
                    "#d5cec1",
                    "#d62828",
                    "#0a6c8f",
                    "#e63946",
                    "#c1121f",
                    "#1b1b1b",
                    ["#7fa8d0", "#3f9147", "#d9a441", "#a8682f"],
                ),
            };
        Self {
            background: over.background.clone().unwrap_or_else(|| background.into()),
            foreground: over.foreground.clone().unwrap_or_else(|| foreground.into()),
            muted: over.muted.clone().unwrap_or_else(|| muted.into()),
            accent: over.accent.clone().unwrap_or_else(|| accent.into()),
            solar: over.solar.clone().unwrap_or_else(|| solar.into()),
            weekend: over.weekend.clone().unwrap_or_else(|| weekend.into()),
            week_marks: over.week_marks.clone().unwrap_or_else(|| week_marks.into()),
            border: over.border.clone().unwrap_or_else(|| border.into()),
            seasons: over.seasons.clone().unwrap_or(seasons.map(String::from)),
        }
    }
}

/// Partial palette. `deny_unknown_fields` so a typo is a parse error, not silence.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Overrides {
    pub background: Option<String>,
    pub foreground: Option<String>,
    pub muted: Option<String>,
    pub accent: Option<String>,
    pub solar: Option<String>,
    pub weekend: Option<String>,
    pub week_marks: Option<String>,
    pub border: Option<String>,
    pub seasons: Option<[String; 4]>,
}

/// Where on the Earth. No platform will tell you this, so it is configuration.
/// Needed only for sunrise and sunset.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    /// Degrees north, -90..90.
    pub lat: f64,
    /// Degrees east, -180..180.
    pub lon: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Font {
    pub family: String,
    /// Fraction of the canvas side, so the whole thing scales with `size`.
    pub size: f64,
    pub weight: u16,
    /// Explicit font file for SVG rasterisation. Falls back to `$WYCCA_FONT`,
    /// then to whatever fontdb can find. The core never reads this.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            family: "sans-serif".into(),
            size: 0.02,
            weight: 400,
            path: None,
        }
    }
}

/// A fixed-length span, written `"2w"`, `"90d"`, `"1h"`, bare units mean one of them.
/// Calendar-accurate units (months, years) are deliberately absent: they have no
/// fixed length, so an interval ring cannot index them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub count: u64,
    pub unit: Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
}

impl Unit {
    pub fn millis(self) -> u64 {
        match self {
            Self::Second => 1_000,
            Self::Minute => 60_000,
            Self::Hour => 3_600_000,
            Self::Day => 86_400_000,
            Self::Week => 604_800_000,
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "s" | "sec" | "second" => Some(Self::Second),
            "m" | "min" | "minute" => Some(Self::Minute),
            "h" | "hour" => Some(Self::Hour),
            "d" | "day" => Some(Self::Day),
            "w" | "week" => Some(Self::Week),
            _ => None,
        }
    }
}

impl FromStr for Span {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        let (count, unit) = s.split_at(split);
        let count = if count.is_empty() {
            1
        } else {
            count
                .parse()
                .map_err(|_| format!("bad span count in {s:?}"))?
        };
        let unit = Unit::parse(unit.trim()).ok_or_else(|| format!("bad span unit in {s:?}"))?;
        Ok(Self { count, unit })
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = match self.unit {
            Unit::Second => "s",
            Unit::Minute => "m",
            Unit::Hour => "h",
            Unit::Day => "d",
            Unit::Week => "w",
        };
        write!(f, "{}{unit}", self.count)
    }
}

impl Serialize for Span {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Span {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(de::Error::custom)
    }
}

/// A one-off radial mark on an [`IntervalRing`], by absolute time.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Marker {
    /// Milliseconds since the unix epoch.
    pub at_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RingSpec {
    Hour24(HourRing),
    Week(WeekRing),
    Month(MonthRing),
    Year(YearRing),
    Interval(IntervalRing),
}

impl RingSpec {
    /// Set to pin the ring to an exact outer radius instead of the layout.
    pub fn radius(&self) -> Option<f64> {
        match self {
            Self::Hour24(r) => r.radius,
            Self::Week(r) => r.radius,
            Self::Month(r) => r.radius,
            Self::Year(r) => r.radius,
            Self::Interval(r) => r.radius,
        }
    }

    pub fn thickness(&self) -> f64 {
        match self {
            Self::Hour24(r) => r.thickness,
            Self::Week(r) => r.thickness,
            Self::Month(r) => r.thickness,
            Self::Year(r) => r.thickness,
            Self::Interval(r) => r.thickness,
        }
    }
}

/// 24 hours over the full circle, not 12. The hand sweeps continuously.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HourRing {
    pub radius: Option<f64>,
    pub thickness: f64,
    pub offset_deg: f64,
    pub ticks: bool,
    /// Draw a longer tick every n hours, and label those only.
    pub major_every: u32,
    pub labels: bool,
    pub label_major_only: bool,
    pub hand: bool,
    /// Fraction of the half-canvas the hand starts at. 0 = from the centre.
    pub hand_inner: f64,
    pub hand_dot: bool,
    /// Sunrise and sunset marks, when the config has a `location`.
    pub sun: bool,
}

impl Default for HourRing {
    fn default() -> Self {
        Self {
            radius: None,
            thickness: 0.012,
            offset_deg: 0.0,
            ticks: true,
            major_every: 6,
            labels: true,
            label_major_only: true,
            hand: true,
            hand_inner: 0.0,
            hand_dot: true,
            sun: true,
        }
    }
}

/// Week from Monday, weekend in [`Palette::weekend`], today marked.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeekRing {
    pub radius: Option<f64>,
    pub thickness: f64,
    pub offset_deg: f64,
    /// Monday at the top unless the ring is rotated.
    pub monday_at_top: bool,
    pub today_marker: bool,
}

impl Default for WeekRing {
    fn default() -> Self {
        Self {
            radius: None,
            thickness: 0.085,
            offset_deg: 0.0,
            monday_at_top: true,
            today_marker: true,
        }
    }
}

/// The 12 months, proportional to their length, running counter-clockwise like
/// the year ring so both are read the same way.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MonthRing {
    pub radius: Option<f64>,
    pub thickness: f64,
    pub offset_deg: f64,
    /// The elapsed part of the current month is drawn brighter than the rest.
    pub progress: bool,
    pub today_marker: bool,
}

impl Default for MonthRing {
    fn default() -> Self {
        Self {
            radius: None,
            thickness: 0.045,
            offset_deg: 0.0,
            progress: true,
            today_marker: true,
        }
    }
}

/// The year, counter-clockwise, seasons symmetric about the cardinal axes.
/// Northern hemisphere months; `offset_deg` rotates the whole thing.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct YearRing {
    pub radius: Option<f64>,
    pub thickness: f64,
    pub offset_deg: f64,
    pub week_ticks: bool,
    pub month_ticks: bool,
    pub month_labels: bool,
    /// Equinoxes and solstices, see [`crate::sun`].
    pub year_events: bool,
    /// Whole-day events from `Config::events`, drawn across the band.
    pub events: bool,
    pub event_labels: bool,
    pub today_marker: bool,
}

impl Default for YearRing {
    fn default() -> Self {
        Self {
            radius: None,
            thickness: 0.13,
            offset_deg: 0.0,
            week_ticks: true,
            month_ticks: true,
            month_labels: true,
            year_events: true,
            events: true,
            event_labels: true,
            today_marker: true,
        }
    }
}

/// The generic "n x something" ring: a sprint, a release train, a lunar month once
/// new-moon instants are what feed the markers.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct IntervalRing {
    pub radius: Option<f64>,
    pub thickness: f64,
    pub offset_deg: f64,
    pub every: Span,
    /// Shift the cycle by this many millis, so "quarters" can start in February.
    pub phase_ms: i64,
    pub progress: bool,
    pub markers: Vec<Marker>,
    pub labels: bool,
}

impl Default for IntervalRing {
    fn default() -> Self {
        Self {
            radius: None,
            thickness: 0.04,
            offset_deg: 0.0,
            every: Span {
                count: 1,
                unit: Unit::Week,
            },
            phase_ms: 0,
            progress: true,
            markers: Vec::new(),
            labels: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Canvas side in pixels. Everything else is a fraction of this.
    pub size: u32,
    /// `None` falls back to the theme background, `"none"` means transparent.
    pub background: Option<String>,
    pub theme: Theme,
    pub colors: Overrides,
    pub font: Font,
    /// Rings are laid out inner to outer in this order. Each one's `radius` may
    /// pin it in place; otherwise see `spacing`.
    pub rings: Vec<RingSpec>,
    /// Whole days worth marking, from an imported calendar or written by hand.
    /// The year ring draws them; the browser keeps the raw `.ics` in local storage
    /// and parses it there, so nothing here ever touches the network.
    pub events: Vec<crate::events::Event>,
    /// Only needed for sunrise and sunset.
    pub location: Option<Location>,
    /// Annulus the automatic layout spreads rings over.
    pub inner_radius: f64,
    pub outer_radius: f64,
    /// 0 spaces the rings evenly, 1 squeezes the inner ones together and pushes
    /// the outer ones out (`t^(1 + 2 * spacing)`). One slider's worth of curve.
    pub spacing: f64,
    /// Offset of the wall clock from UTC, in millis. Solar events are global
    /// instants; this is what lands them on the right hour. The CLI fills it in
    /// from the system zone, so it is only worth setting by hand when rendering
    /// someone else's clock.
    pub utc_offset_ms: Option<i64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            size: 1024,
            background: None,
            theme: Theme::Dark,
            colors: Overrides::default(),
            font: Font::default(),
            rings: vec![
                RingSpec::Hour24(HourRing::default()),
                RingSpec::Week(WeekRing::default()),
                RingSpec::Month(MonthRing::default()),
                RingSpec::Year(YearRing::default()),
            ],
            events: Vec::new(),
            location: None,
            inner_radius: 0.0,
            outer_radius: 0.9,
            spacing: 0.01,
            utc_offset_ms: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_parse_and_roundtrip() {
        assert_eq!(
            "2w".parse(),
            Ok(Span {
                count: 2,
                unit: Unit::Week
            })
        );
        assert_eq!(
            "h".parse(),
            Ok(Span {
                count: 1,
                unit: Unit::Hour
            })
        );
        assert!("2 weeks".parse::<Span>().is_err());
        assert_eq!(
            serde_json::to_string(&"2w".parse::<Span>().unwrap()).unwrap(),
            "\"2w\""
        );
    }

    #[test]
    fn unknown_field_is_an_error() {
        assert!(
            serde_json::from_str::<Config>(r#"{"rings":[{"type":"week","radiuss":1}]}"#).is_err()
        );
    }
}
