//! Config schema. Everything a ring draws is data; nothing here touches the clock.

use std::path::PathBuf;

use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    /// The other half of `#[serde(rename_all = "snake_case")]`, spelled out so the
    /// form can key the resolved palettes by name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

/// Winter, spring, summer, autumn.
#[derive(Debug, Clone, Serialize)]
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
    /// Daylight hours on the hour ring. On the dark theme this is what makes the
    /// lit half of the clock visible against a dark wallpaper.
    pub day: String,
    /// The dark hours on the hour ring: night is whatever the background is not,
    /// so on the dark theme it disappears into it and on the light theme it is
    /// painted dark.
    pub night: String,
    pub seasons: [String; 4],
}

impl Palette {
    pub fn resolve(theme: Theme, over: &Overrides) -> Self {
        let (
            background,
            foreground,
            muted,
            accent,
            solar,
            weekend,
            week_marks,
            border,
            day,
            night,
            seasons,
        ) = match theme {
            Theme::Dark => (
                "#0e1116",
                "#e8e6e1",
                "#39404d",
                "#ff9f1c",
                "#4cc9f0",
                "#c1121f",
                "#e63946",
                "#12100e",
                "#e8e6e1",
                "#0e1116",
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
                "#f4f1ea",
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
            day: over.day.clone().unwrap_or_else(|| day.into()),
            night: over.night.clone().unwrap_or_else(|| night.into()),
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
    pub day: Option<String>,
    pub night: Option<String>,
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

/// What one turn of a ring covers.
///
/// Every angle on a ring is a fraction of its scale's current period, so layers
/// on one ring — and two rings of the same scale — can never disagree about
/// where anything sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scale {
    /// The current day, midnight to midnight.
    Hour24,
    /// Monday to Monday.
    Week,
    /// The first of the month to the first of the next.
    Month,
    /// The calendar year, January to January, so the seasons sit symmetric
    /// about the cardinal axes with each season's three months inside its
    /// quadrant and January at the top.
    Year,
}

impl Scale {
    /// +1 runs clockwise, -1 counter-clockwise. Only the year runs backwards, so
    /// January follows December the way a calendar page reads.
    pub(crate) fn dir(self) -> f64 {
        match self {
            Self::Year => -1.0,
            _ => 1.0,
        }
    }

    /// The span of this scale that contains `now`.
    pub fn period(&self, now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
        match self {
            Self::Hour24 => {
                let midnight = now.date_naive().and_time(NaiveTime::MIN).and_utc();
                (midnight, midnight + chrono::Duration::days(1))
            }
            Self::Week => {
                let mut monday = now.date_naive().and_time(NaiveTime::MIN).and_utc();
                while monday.weekday().num_days_from_monday() != 0 {
                    monday -= chrono::Duration::days(1);
                }
                (monday, monday + chrono::Duration::days(7))
            }
            Self::Month => {
                let (y, m) = (now.year(), now.month());
                (
                    date(y, m, 1),
                    if m == 12 {
                        date(y + 1, 1, 1)
                    } else {
                        date(y, m + 1, 1)
                    },
                )
            }
            Self::Year => (date(now.year(), 1, 1), date(now.year() + 1, 1, 1)),
        }
    }
}

/// How a whole-day event is drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventStyle {
    /// A red day sector, the way the old month ring drew its holidays.
    Sector,
    /// A full crossing line for a day off, a short notch for a busy day.
    #[default]
    Notch,
}

/// What a ring draws. A ring is these flags; no ring type is special anymore.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Layers {
    pub ticks: bool,
    pub tick_labels: bool,
    pub label_major_only: bool,
    /// A longer tick every n hours; the labelled ones, when `tick_labels`.
    pub major_every: u32,
    pub hand: bool,
    /// Fraction of the half-canvas the hand starts at. 0 = from the centre.
    pub hand_inner: f64,
    pub hand_dot: bool,
    /// Sunrise and sunset marks, when the config has a `location`.
    pub sun: bool,
    /// The dark hours, sunset to sunrise, in the night colour.
    pub night: bool,
    /// The lit hours, sunrise to sunset, in the day colour.
    pub day: bool,
    /// One sector per day, the weekend days in the weekend colour.
    pub day_sectors: bool,
    /// The twelve months, each as wide as it is long.
    pub month_sectors: bool,
    /// A tick on every day, so a month reads as a run of days.
    pub day_borders: bool,
    /// Saturday and Sunday, in the weekend red. Long ticks, so a week reads at
    /// a glance.
    pub weekends: bool,
    /// The day-of-month number on every day.
    pub day_labels: bool,
    pub month_ticks: bool,
    /// The month number at each month's middle.
    pub month_labels: bool,
    /// The four seasons, each holding its three months.
    pub seasons: bool,
    pub week_ticks: bool,
    /// Equinoxes and solstices, see [`crate::sun`].
    pub year_events: bool,
    /// Whole-day events from `Config::events`.
    pub events: bool,
    pub event_style: EventStyle,
    pub event_labels: bool,
    /// The elapsed part of the ring's period, brighter than the rest.
    pub progress: bool,
    pub today_marker: bool,
}

/// One ring: a scale, a place on the canvas, and the layers drawn on it.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ring {
    /// Pin the ring to an exact outer radius instead of letting `layout()` place it.
    #[serde(default)]
    pub radius: Option<f64>,
    #[serde(default)]
    pub thickness: f64,
    #[serde(default)]
    pub offset_deg: f64,
    #[serde(default)]
    pub reversed: bool,
    pub scale: Scale,
    #[serde(default)]
    pub layers: Layers,
}

impl Ring {
    /// A fresh ring at `scale`: the geometry every ring shares, plus the layers
    /// that make the scale worth adding as-is. Everything else is a checkbox.
    pub fn new(scale: Scale) -> Self {
        let mut layers = Layers {
            today_marker: true,
            ..Default::default()
        };
        match scale {
            Scale::Hour24 => {
                layers = Layers {
                    ticks: true,
                    tick_labels: true,
                    label_major_only: true,
                    major_every: 6,
                    hand: true,
                    hand_dot: true,
                    sun: true,
                    night: true,
                    day: true,
                    ..layers
                };
            }
            Scale::Week => layers.day_sectors = true,
            Scale::Month => {
                layers.day_borders = true;
                layers.weekends = true;
            }
            Scale::Year => layers.month_sectors = true,
        }
        Self {
            radius: None,
            thickness: 0.04,
            offset_deg: 0.0,
            reversed: false,
            scale,
            layers,
        }
    }
}

pub(crate) fn date(y: i32, m: u32, d: u32) -> DateTime<Utc> {
    NaiveDate::from_ymd_opt(y, m, d)
        .unwrap_or_default()
        .and_time(NaiveTime::MIN)
        .and_utc()
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
    pub rings: Vec<Ring>,
    /// Whole days worth marking, from an imported calendar or written by hand.
    /// The year-scale rings draw them; the browser keeps the raw `.ics` in local
    /// storage and parses it there, so nothing here ever touches the network.
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
                Ring {
                    thickness: 0.012,
                    ..Ring::new(Scale::Hour24)
                },
                Ring {
                    thickness: 0.085,
                    ..Ring::new(Scale::Week)
                },
                Ring {
                    thickness: 0.045,
                    scale: Scale::Month,
                    layers: Layers {
                        day_borders: true,
                        day_labels: true,
                        weekends: true,
                        today_marker: true,
                        ..Default::default()
                    },
                    ..Ring::new(Scale::Month)
                },
                Ring {
                    thickness: 0.13,
                    offset_deg: 15.0,
                    layers: Layers {
                        month_ticks: true,
                        month_labels: true,
                        year_events: true,
                        events: true,
                        event_labels: true,
                        seasons: true,
                        week_ticks: true,
                        weekends: true,
                        today_marker: true,
                        ..Default::default()
                    },
                    ..Ring::new(Scale::Year)
                },
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
    fn unknown_field_is_an_error() {
        assert!(
            serde_json::from_str::<Config>(r#"{"rings":[{"scale":"week","radiuss":1}]}"#).is_err()
        );
    }

    #[test]
    fn the_year_period_is_the_calendar_year() {
        let (start, end) = Scale::Year.period(date(2027, 6, 15));
        assert_eq!((start.year(), start.month(), start.day()), (2027, 1, 1));
        assert_eq!((end.year(), end.month(), end.day()), (2028, 1, 1));
        let (start, _end) = Scale::Year.period(date(2027, 1, 1));
        assert_eq!((start.year(), start.month(), start.day()), (2027, 1, 1));
    }

    #[test]
    fn the_month_period_is_the_current_month() {
        let (start, end) = Scale::Month.period(date(2026, 10, 13));
        assert_eq!((start.year(), start.month(), start.day()), (2026, 10, 1));
        assert_eq!((end.year(), end.month(), end.day()), (2026, 11, 1));
    }

    #[test]
    fn the_week_period_starts_on_monday() {
        // 2026-10-14 is a Wednesday.
        let (start, end) = Scale::Week.period(date(2026, 10, 14));
        assert_eq!(start.weekday().num_days_from_monday(), 0);
        assert_eq!((end - start).num_days(), 7);
    }
}
