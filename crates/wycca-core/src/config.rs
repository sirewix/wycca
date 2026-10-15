//! Config schema. Everything a ring draws is data; nothing here touches the clock.
//!
//! A ring is one continuous span of time — its [`Scale`] — divided into spans.
//! Every span comes from a [`Layer`], and a layer is two things: where its spans
//! come from, and how to draw them. Those are the only two primitives.

use std::path::PathBuf;

use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    /// By season name, so a layer can ask for `"winter"` the way it asks for
    /// `"accent"`.
    pub spring: String,
    pub summer: String,
    pub autumn: String,
    pub winter: String,
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
            winter,
            spring,
            summer,
            autumn,
        ) = match theme {
            Theme::Dark => (
                "#0e1116", "#e8e6e1", "#39404d", "#ff9f1c", "#4cc9f0", "#c1121f", "#e63946",
                "#12100e", "#e8e6e1", "#0e1116", "#cfe0f3", "#63b862", "#e0c56e", "#c08552",
            ),
            Theme::Light => (
                "#f4f1ea", "#1b1b1b", "#d5cec1", "#d62828", "#0a6c8f", "#e63946", "#c1121f",
                "#1b1b1b", "#f4f1ea", "#1b1b1b", "#7fa8d0", "#3f9147", "#d9a441", "#a8682f",
            ),
        };
        let pick = |set: Option<String>, or: &str| set.unwrap_or_else(|| or.into());
        Self {
            background: pick(over.background.clone(), background),
            foreground: pick(over.foreground.clone(), foreground),
            muted: pick(over.muted.clone(), muted),
            accent: pick(over.accent.clone(), accent),
            solar: pick(over.solar.clone(), solar),
            weekend: pick(over.weekend.clone(), weekend),
            week_marks: pick(over.week_marks.clone(), week_marks),
            border: pick(over.border.clone(), border),
            day: pick(over.day.clone(), day),
            night: pick(over.night.clone(), night),
            spring: pick(over.spring.clone(), spring),
            summer: pick(over.summer.clone(), summer),
            autumn: pick(over.autumn.clone(), autumn),
            winter: pick(over.winter.clone(), winter),
        }
    }

    /// Every slot name, which is what `color` answers to.
    pub const SLOTS: &'static [&'static str] = &[
        "background",
        "foreground",
        "muted",
        "accent",
        "solar",
        "weekend",
        "week_marks",
        "border",
        "day",
        "night",
        "spring",
        "summer",
        "autumn",
        "winter",
    ];

    /// A layer's colour is a palette slot name or a `#rrggbb`, so one string
    /// covers both a theme-aware default and a hand-picked colour. An unknown
    /// name is passed through and renders as nothing, which is what a typo
    /// should look like.
    pub fn color<'a>(&'a self, name: &'a str) -> &'a str {
        match name {
            "background" => &self.background,
            "foreground" => &self.foreground,
            "muted" => &self.muted,
            "accent" => &self.accent,
            "solar" => &self.solar,
            "weekend" => &self.weekend,
            "week_marks" => &self.week_marks,
            "border" => &self.border,
            "day" => &self.day,
            "night" => &self.night,
            "spring" => &self.spring,
            "summer" => &self.summer,
            "autumn" => &self.autumn,
            "winter" => &self.winter,
            other => other,
        }
    }
}

/// Partial palette. `deny_unknown_fields` so a typo is a parse error, not silence.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    /// By season name, so a layer can ask for `"winter"` the way it asks for
    /// `"accent"`, and every slot is a field of its own rather than an index
    /// into an array nobody can read.
    pub spring: Option<String>,
    pub summer: Option<String>,
    pub autumn: Option<String>,
    pub winter: Option<String>,
}

/// Where on the Earth. No platform will tell you this, so it is configuration.
/// Needed only for sunrise and sunset.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Location {
    /// Degrees north, -90..90.
    pub lat: f64,
    /// Degrees east, -180..180.
    pub lon: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

/// The unit a periodic pattern counts in.
///
/// Each unit counts from its own epoch, chosen so that index zero is the phase
/// zero: midnight, midnight on a Monday, January 1970. That is what makes
/// `shift` mean the same thing on every ring — a weekday on a day ring is still
/// a Saturday on a year ring, because both count from the same Monday.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Year,
    Month,
    Week,
    /// The default, because a bare periodic layer is one span per day.
    #[default]
    Day,
    Hour,
    Minute,
}

impl Unit {
    pub fn name(self) -> &'static str {
        match self {
            Self::Year => "year",
            Self::Month => "month",
            Self::Week => "week",
            Self::Day => "day",
            Self::Hour => "hour",
            Self::Minute => "minute",
        }
    }

    /// Short form for a layer's title, where space is the scarce resource.
    fn abbr(self) -> &'static str {
        match self {
            Self::Year => "y",
            Self::Month => "mo",
            Self::Week => "w",
            Self::Day => "d",
            Self::Hour => "h",
            Self::Minute => "min",
        }
    }

    /// The phase `shift` selects, in the words a person would use. A title is
    /// the only place the words matter, so a phase reads better than a number.
    pub fn phase(self, shift: i64) -> String {
        match self {
            Self::Day => DAYS[shift.rem_euclid(7) as usize].to_owned(),
            Self::Month => MONTHS[shift.rem_euclid(12) as usize].to_owned(),
            Self::Hour => shift.rem_euclid(24).to_string(),
            Self::Minute => format!("min {}", shift.rem_euclid(60)),
            Self::Week => format!("week {shift}"),
            Self::Year => (1970 + shift).to_string(),
        }
    }
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
const DAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/// The phase a pattern starts at: an index into `unit`'s cycle, or the name of
/// one. `shift: sat` is a Saturday and `shift: dec` is a December; the numbers
/// stay for the units with no names, so `shift: 0` is midnight or January.
///
/// Untagged on purpose: one field takes `5` and `"sat"` alike, which is what
/// lets a config say `shift: sat` while the page writes a plain number for an
/// hour.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum Shift {
    Name(String),
    Index(i64),
}

impl Default for Shift {
    /// Phase zero, which is midnight and January.
    fn default() -> Self {
        Self::Index(0)
    }
}

impl Shift {
    /// The phase, as an index into `unit`'s cycle. A name belongs to the unit it
    /// names — a weekday to days, a month to months — and means phase zero
    /// anywhere else, where the cycle has no name in it.
    pub fn index(&self, unit: Unit) -> i64 {
        match self {
            Self::Index(n) => *n,
            Self::Name(name) => {
                let table = match unit {
                    Unit::Day => &DAYS[..],
                    Unit::Month => &MONTHS[..],
                    _ => &[][..],
                };
                table.iter().position(|t| *t == name).unwrap_or(0) as i64
            }
        }
    }

    /// The phase read back, as a name where there is one.
    pub fn name(&self, unit: Unit) -> String {
        match self {
            Self::Name(name) => name.clone(),
            Self::Index(n) => unit.phase(*n),
        }
    }
}

impl From<i64> for Shift {
    fn from(n: i64) -> Self {
        Self::Index(n)
    }
}

impl From<&str> for Shift {
    fn from(name: &str) -> Self {
        Self::Name(name.into())
    }
}

/// The feed a platform puts the calendar it was handed into: what `--events` and
/// the browser's file picker both reach for, and what the year preset's holidays
/// layer points at.
pub const DEFAULT_FEED: &str = "holidays";

/// Where a span's fill sits inside the ring's thickness. Both are trims: `inner`
/// is taken off the ring's inner edge and `outer` off its outer edge, as a
/// percentage of the ring's thickness. `0`/`0` is the whole ring, `55`/`0` is its
/// outer half, and a negative number goes past the edge — which is what a mark
/// that has to be seen from across the room does.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Offsets {
    pub inner: f64,
    pub outer: f64,
}

impl Default for Offsets {
    fn default() -> Self {
        Self {
            inner: 0.0,
            outer: 0.0,
        }
    }
}

/// Where a label sits against a fill: `in` is inside its near edge, `at` is on
/// it, `out` is past its far edge.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Where {
    #[default]
    In,
    At,
    Out,
}

/// Along the span: a tick's label stands on the tick (`start`), a day's number in
/// the middle of its day (`middle`), a month at its far edge (`end`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    #[default]
    Start,
    Middle,
    End,
}

/// What text a span carries, if any. The source decides the words; this decides
/// whether they are drawn, where across the ring, and which end they stand at.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Label {
    /// Across the ring: inside the fill, on it, or past its far edge.
    pub at: Where,
    /// Along the span.
    pub anchor: Anchor,
    /// Palette slot or `#rrggbb`.
    pub color: String,
    /// Multiple of the canvas `font.size`.
    pub size: f64,
}

impl Default for Label {
    fn default() -> Self {
        Self {
            at: Where::In,
            anchor: Anchor::Start,
            color: "foreground".into(),
            size: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BorderKind {
    /// Across the span's leading edge.
    #[default]
    Line,
    /// From the centre out to the ring: a clock hand.
    Hand,
}

/// The stroke a span draws. One at its leading edge, which is a tick for a
/// zero-length span, a rule between two neighbours, or a notch pointing at a
/// holiday.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Border {
    pub kind: BorderKind,
    /// Palette slot or `#rrggbb`.
    pub color: String,
    /// Stroke width, as a fraction of the half-canvas.
    pub width: f64,
    /// Where a hand starts, as a fraction of the half-canvas. Only `hand` reads it.
    pub inner: f64,
}

impl Default for Border {
    fn default() -> Self {
        Self {
            kind: BorderKind::Line,
            // A rule between two neighbours is drawn in the colour of the paper
            // it is drawn on, so a border you add without a colour divides what
            // is under it and disappears where nothing is.
            color: "background".into(),
            width: RULE,
            inner: 0.0,
        }
    }
}

/// How a span is drawn. Every part of it is optional: a layer with nothing set
/// draws nothing, which is how a span of colour alone differs from a bare tick.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Disp {
    /// Fill the span. `None` for no fill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// Stroke at the span's leading edge. `None` for none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<Label>,
    /// Where inside the ring's thickness the fill sits. `None` fills it all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offsets: Option<Offsets>,
    /// Degrees trimmed off each end of the fill, so neighbours read as separate
    /// sectors instead of one block.
    pub gap: f64,
}

impl Disp {
    /// The ring's radial extent for this layer, from the offsets.
    pub fn band(&self, r_in: f64, r_out: f64) -> (f64, f64) {
        match self.offsets {
            Some(o) => (
                r_in + (r_out - r_in) * o.inner / 100.0,
                r_out - (r_out - r_in) * o.outer / 100.0,
            ),
            None => (r_in, r_out),
        }
    }
}

/// The sun's own spans and instants. Nothing here is periodic: it needs a place
/// and a day.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Solar {
    /// Sunrise to sunset.
    #[default]
    Day,
    /// Sunset to the next sunrise.
    Night,
    Sunrise,
    Sunset,
    Equinox,
    Solstice,
}

impl Solar {
    pub fn name(self) -> &'static str {
        match self {
            Self::Day => "daylight",
            Self::Night => "night",
            Self::Sunrise => "sunrise",
            Self::Sunset => "sunset",
            Self::Equinox => "equinox",
            Self::Solstice => "solstice",
        }
    }
}

/// One thing a ring draws: where its spans come from, and how to draw them.
///
/// A layer is one row of the config and one block of the form, so a new thing to
/// draw is a new variant here plus a block in `rings::spans` — there is no
/// per-layer-type drawing code anywhere else.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Layer {
    /// Spans a pattern generates. `amount` units long, starting `shift` units into
    /// the unit's cycle and repeating every `every` units. `amount: 0` is an
    /// instant: a tick, a marker, or somewhere to put a label. The defaults are
    /// "every day", so a bare periodic layer is one span per day.
    Periodic {
        #[serde(default)]
        unit: Unit,
        #[serde(default = "one")]
        amount: i64,
        #[serde(default = "one")]
        every: i64,
        #[serde(default)]
        shift: Shift,
        #[serde(default)]
        disp: Disp,
    },
    /// Whole days from a named `.ics` feed. `only` narrows it to days off
    /// (`free`) or days on (`busy`); unset draws both.
    Ics {
        #[serde(default)]
        source: String,
        #[serde(default)]
        only: Option<crate::events::Kind>,
        #[serde(default)]
        disp: Disp,
    },
    #[serde(rename = "solar")]
    Solar {
        #[serde(default)]
        what: Solar,
        #[serde(default)]
        disp: Disp,
    },
    /// Where "now" is. An instant, like a tick, only it is you.
    #[serde(rename = "now")]
    Now {
        #[serde(default)]
        disp: Disp,
    },
}

fn one() -> i64 {
    1
}

impl Layer {
    /// Every kind, as the config and the form name it.
    pub const KINDS: &'static [&'static str] = &["periodic", "ics", "solar", "now"];

    pub fn periodic(unit: Unit, amount: i64, every: i64, shift: impl Into<Shift>) -> Self {
        Self::Periodic {
            unit,
            amount,
            every,
            shift: shift.into(),
            disp: Disp::default(),
        }
    }

    pub fn ics(source: &str, only: Option<crate::events::Kind>) -> Self {
        Self::Ics {
            source: source.into(),
            only,
            disp: Disp::default(),
        }
    }

    pub fn solar(what: Solar) -> Self {
        Self::Solar {
            what,
            disp: Disp::default(),
        }
    }

    pub fn now() -> Self {
        Self::Now {
            disp: Disp::default(),
        }
    }

    /// A new layer of `kind`, so a UI adds one by name instead of writing a
    /// half-filled object. The constructors above are where the starting values
    /// are chosen; this only dispatches on the name.
    pub fn new(kind: &str) -> Option<Self> {
        Some(match kind {
            "periodic" => Self::periodic(Unit::Day, 1, 1, 0),
            "ics" => Self::ics("", None),
            "solar" => Self::solar(Solar::Day),
            "now" => Self::now(),
            _ => return None,
        })
    }

    pub fn disp(&self) -> &Disp {
        match self {
            Self::Periodic { disp, .. }
            | Self::Ics { disp, .. }
            | Self::Solar { disp, .. }
            | Self::Now { disp } => disp,
        }
    }

    pub fn disp_mut(&mut self) -> &mut Disp {
        match self {
            Self::Periodic { disp, .. }
            | Self::Ics { disp, .. }
            | Self::Solar { disp, .. }
            | Self::Now { disp } => disp,
        }
    }

    /// The layer card's title: the source, spelled the way the config spells it.
    /// The core owns this so the two UIs and the config cannot drift apart.
    pub fn title(&self) -> String {
        match self {
            Self::Periodic {
                unit,
                amount,
                every,
                shift,
                ..
            } => {
                let (n, e) = (*amount, *every);
                // The two everyday cases say so in words: "every day" is a layer
                // anyone can place, and "1d from mon" is not.
                match (n, e) {
                    (1, 1) => return format!("every {}", unit.name()),
                    (0, 1) => return format!("a mark every {}", unit.name()),
                    _ => {}
                }
                format!(
                    "{n}{u} every {e}{u} from {phase}",
                    u = unit.abbr(),
                    phase = shift.name(*unit)
                )
            }
            Self::Ics { source, only, .. } => match (source.is_empty(), only) {
                (true, _) => "ics".into(),
                (false, Some(k)) => format!("{} {}", source, k.name()),
                (false, None) => source.clone(),
            },
            Self::Solar { what, .. } => what.name().into(),
            Self::Now { .. } => "now".into(),
        }
    }
}

/// One ring: a scale, a place on the canvas, and the layers drawn on it.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    pub layers: Vec<Layer>,
}

/// A layer that fills the ring, in `color`, for every span its source produces.
fn fill(layer: Layer, color: &str) -> Layer {
    let mut layer = layer;
    layer.disp_mut().background = Some(color.into());
    layer
}

/// A layer whose only job is a radial stroke across `inner..outer` per cent of
/// the ring's thickness. `amount: 0` gives a tick rather than a notch.
fn line(layer: Layer, color: &str, width: f64, inner: f64, outer: f64) -> Layer {
    let mut layer = layer;
    layer.disp_mut().border = Some(Border {
        color: color.into(),
        width,
        ..Border::default()
    });
    layer.disp_mut().offsets = Some(Offsets { inner, outer });
    layer
}

/// A layer whose only job is text, placed clear of the fill by its own height.
fn caption(layer: Layer, color: &str, at: Where, anchor: Anchor, size: f64) -> Layer {
    let mut layer = layer;
    layer.disp_mut().label = Some(Label {
        at,
        anchor,
        color: color.into(),
        size,
    });
    layer
}

/// The clock hand: from the centre out through the ring.
fn hand() -> Layer {
    let mut layer = Layer::now();
    layer.disp_mut().border = Some(Border {
        kind: BorderKind::Hand,
        color: "accent".into(),
        width: MARK,
        inner: 0.0,
    });
    layer
}

/// Where "now" is, as a rule across the whole ring and a little past it: a rule
/// that stops at the ring's own edge is a dot on a thin ring.
fn today() -> Layer {
    line(Layer::now(), "accent", MARK * 2.0, -10.0, -10.0)
}

/// A stroke a mark draws: thick enough to see, and standing out past the ring.
fn mark(layer: Layer, color: &str) -> Layer {
    line(layer, color, MARK, -10.0, -10.0)
}

/// Width of a mark — now, a solstice, an hour — as a fraction of the
/// half-canvas. Big enough to see: a mark on a thin ring that stops at the ring's
/// own edge is a dot.
const MARK: f64 = 0.004;
/// The hairline between two neighbours: a day, a month, a season edge. A
/// year has 365 of these, so it is thinner than a hand would notice.
const RULE: f64 = 0.001;

impl Ring {
    /// A fresh ring at `scale`: the layers that make that scale worth adding
    /// as-is. A UI asks the core for one of these rather than inventing values.
    pub fn new(scale: Scale) -> Self {
        use Solar::{Day, Equinox, Night, Solstice, Sunrise, Sunset};
        use Unit::{Day as D, Hour as H, Month as M, Week as W};
        let mut ring = Self {
            radius: None,
            thickness: 0.04,
            offset_deg: 0.0,
            reversed: false,
            scale,
            layers: Vec::new(),
        };
        match scale {
            Scale::Hour24 => {
                ring.layers = vec![
                    fill(Layer::solar(Day), "day"),
                    fill(Layer::solar(Night), "night"),
                    // An hour is a tick; every sixth hour is a mark.
                    line(Layer::periodic(H, 0, 1, 0), "muted", MARK, 55.0, 0.0),
                    line(Layer::periodic(H, 0, 6, 0), "muted", MARK * 2.0, 0.0, 0.0),
                    caption(
                        Layer::periodic(H, 0, 6, 0),
                        "foreground",
                        Where::In,
                        Anchor::Start,
                        1.0,
                    ),
                    mark(Layer::solar(Sunrise), "solar"),
                    mark(Layer::solar(Sunset), "solar"),
                    today(),
                    hand(),
                ];
            }
            Scale::Week => {
                ring.layers = vec![
                    fill(Layer::periodic(D, 1, 1, "mon"), "muted"),
                    fill(Layer::periodic(D, 2, 7, "sat"), "weekend"),
                    // Saturday and Sunday are one span, so the line between them
                    // is Sunday's own edge: a rule in the paper's colour, drawn
                    // over the fill it divides. The line the week turns on is
                    // Monday's edge, and every day has a gap either side of it.
                    mark(Layer::periodic(D, 0, 7, "sun"), "border"),
                    mark(Layer::periodic(W, 0, 1, 0), "border"),
                    today(),
                ];
                ring.layers[0].disp_mut().gap = 0.6;
            }
            Scale::Month => {
                ring.layers = vec![
                    // The days, then the weekends over them, then the numbers on
                    // top of both: a label that is under a fill is no label.
                    line(
                        Layer::periodic(D, 0, 1, "mon"),
                        "border",
                        RULE * 2.0,
                        55.0,
                        10.0,
                    ),
                    fill(Layer::periodic(D, 2, 7, "sat"), "weekend"),
                    caption(
                        Layer::periodic(D, 0, 1, "mon"),
                        "foreground",
                        Where::At,
                        Anchor::Middle,
                        0.7,
                    ),
                    today(),
                ];
            }
            Scale::Year => {
                // Winter starts in December and runs three months past the turn,
                // which is why the seasons shift by 11 and not by 0.
                let season = |shift, colour| {
                    let mut layer = fill(Layer::periodic(M, 3, 12, shift), colour);
                    layer.disp_mut().gap = 0.6;
                    layer
                };
                ring.layers = vec![
                    season(11, "winter"),
                    season(2, "spring"),
                    season(5, "summer"),
                    season(8, "autumn"),
                    // The months between the seasons, the days inside them and
                    // the weeks inside those. Trims, so `55, 10` is 55% to 90% of
                    // the ring's thickness and `0, 70` is its outer third.
                    line(
                        Layer::periodic(M, 0, 1, "jan"),
                        "border",
                        MARK * 2.0,
                        -1.0,
                        -1.0,
                    ),
                    line(Layer::periodic(D, 0, 1, "mon"), "border", RULE, 0.0, 0.0),
                    line(Layer::periodic(W, 0, 1, 0), "week_marks", RULE, 0.0, 70.0),
                    caption(
                        Layer::periodic(M, 0, 1, "jan"),
                        "foreground",
                        Where::Out,
                        Anchor::Middle,
                        1.0,
                    ),
                    fill(Layer::periodic(D, 2, 7, "sat"), "weekend"),
                    mark(Layer::solar(Equinox), "solar"),
                    mark(Layer::solar(Solstice), "solar"),
                    // A holiday is a sector, like a weekend: one day of red across
                    // the ring. It carries no label — on a ring this thin the
                    // names run over each other and over the month numbers, and
                    // a ring is read at a glance, not read out.
                    fill(
                        Layer::ics(DEFAULT_FEED, Some(crate::events::Kind::Free)),
                        "weekend",
                    ),
                    today(),
                ];
            }
        }
        ring
    }
}

pub(crate) fn date(y: i32, m: u32, d: u32) -> DateTime<Utc> {
    NaiveDate::from_ymd_opt(y, m, d)
        .unwrap_or_default()
        .and_time(NaiveTime::MIN)
        .and_utc()
}

/// Keeps the default preset — forty-odd layers of JSON — out of the schema. It is
/// in [`Config::default`] and in `config.json`; a third copy in a generated file is
/// one more thing to regenerate for nothing.
#[cfg(feature = "schema")]
fn no_default(schema: &mut schemars::Schema) {
    schema.remove("default");
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    #[cfg_attr(feature = "schema", schemars(transform = no_default))]
    pub rings: Vec<Ring>,
    /// Whole-day events, by feed name. The CLI reads them from `--events`, the
    /// browser from a file picker; nothing here ever touches the network.
    pub feeds: std::collections::BTreeMap<String, Vec<crate::events::Event>>,
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
                    ..Ring::new(Scale::Month)
                },
                Ring {
                    thickness: 0.13,
                    offset_deg: 15.0,
                    ..Ring::new(Scale::Year)
                },
            ],
            feeds: Default::default(),
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
        for json in [
            r##"{"kind":"periodic","unit":"day","amount":1,"shift":0,"disp":{"bakground":"#"}}"##,
            r#"{"kind":"periodic","amoun":1,"shift":0}"#,
            r#"{"kind":"now","nonsense":true}"#,
            r#"{"kind":"solar","whats":"day"}"#,
        ] {
            assert!(
                serde_json::from_str::<Layer>(json).is_err(),
                "a typo must not be a silent no-op: {json}"
            );
        }
        // And the kinds and units are a closed set, not free text.
        assert!(serde_json::from_str::<Layer>(r#"{"kind":"span"}"#).is_err());
        assert!(
            serde_json::from_str::<Layer>(r#"{"kind":"periodic","unit":"fortnight"}"#).is_err()
        );
    }

    #[test]
    fn a_layer_is_only_a_kind_and_a_disp() {
        let layer: Layer = serde_json::from_str(r#"{"kind":"now"}"#).unwrap();
        assert!(layer.disp().background.is_none());
        assert_eq!(layer.title(), "now");
        assert_eq!(Layer::new("now").unwrap().title(), "now");
        assert!(Layer::new("hand").is_none());
    }

    #[test]
    fn every_kind_can_be_made_and_named() {
        for kind in Layer::KINDS {
            let layer = Layer::new(kind).unwrap();
            assert!(!layer.title().is_empty(), "{kind}");
            let json = serde_json::to_string(&layer).unwrap();
            let back: Layer = serde_json::from_str(&json).unwrap();
            assert_eq!(back.title(), layer.title(), "{kind}");
        }
    }

    #[test]
    fn a_named_shift_is_the_number_it_names() {
        // `shift: sat` and `shift: 5` are the same two days, which is the point
        // of the name: a weekend is not "a two-day span every seven from five".
        for (name, n) in [("mon", 0), ("sat", 5), ("sun", 6)] {
            assert_eq!(Shift::Name(name.into()).index(Unit::Day), n, "{name}");
        }
        assert_eq!(Shift::Name("dec".into()).index(Unit::Month), 11);
        // A name means nothing where the cycle has no names, and says so by
        // being phase zero rather than by being an error.
        assert_eq!(Shift::Name("sat".into()).index(Unit::Hour), 0);
        assert_eq!(Shift::Index(5).index(Unit::Day), 5);

        let weekend = |shift| {
            format!(r#"{{"kind":"periodic","unit":"day","amount":2,"every":7,"shift":{shift}}}"#)
        };
        let named: Layer = serde_json::from_str(&weekend("\"sat\"")).unwrap();
        let numbered: Layer = serde_json::from_str(&weekend("5")).unwrap();
        assert_eq!(named.title(), "2d every 7d from sat");
        assert_eq!(named.title(), numbered.title());
        assert_eq!(
            serde_json::to_string(&named).unwrap(),
            serde_json::to_string(&numbered)
                .unwrap()
                .replace("\"shift\":5", "\"shift\":\"sat\""),
            "the same layer either way round"
        );
    }

    #[test]
    fn colours_are_a_slot_or_a_hex() {
        let palette = Palette::resolve(Theme::Dark, &Overrides::default());
        assert_eq!(palette.color("accent"), palette.accent);
        assert_eq!(palette.color("winter"), palette.winter);
        assert_eq!(palette.color("#123456"), "#123456");
        assert_eq!(palette.color("nonsense"), "nonsense");
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
