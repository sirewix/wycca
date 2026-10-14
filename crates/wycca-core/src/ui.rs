//! The configuration form, as data.
//!
//! The core cannot draw a widget, but it can say what one would look like. The
//! browser and the Android widget each render this JSON with their own controls,
//! so a field is declared once here and never again in a UI.
//!
//! The dotted key strings are the only fragile part of the arrangement: they are
//! the sole link back to `config.rs`, and a typo in one is a dead control that
//! nobody notices until a user reports it. `every_key_is_a_real_config_key` is
//! the guard.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::{Overrides, Palette, Theme};

#[derive(Serialize)]
pub struct Form {
    pub sections: Vec<Section>,
    /// What `Palette::resolve` gives for each theme, so a control that is "auto"
    /// still has a real colour to switch to.
    pub palettes: BTreeMap<&'static str, Palette>,
    /// The one ring type's fields. A ring card is these plus a scale picker.
    pub ring_fields: &'static [Field],
}

#[derive(Serialize)]
pub struct Section {
    pub id: &'static str,
    pub label: &'static str,
    pub fields: Vec<Field>,
}

#[derive(Serialize)]
pub struct Field {
    /// Dotted path into the config, `"colors.accent"`. A ring field's path is
    /// relative to one ring object.
    pub key: &'static str,
    pub label: &'static str,
    /// Null in the config means "let the renderer decide", which is what the
    /// checkbox next to the control says.
    #[serde(skip_serializing_if = "is_off")]
    pub optional: bool,
    #[serde(flatten)]
    pub kind: Kind,
}

/// Everything a control can be. Nothing here knows what a widget looks like.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    Bool,
    /// `step` 1 is an integer, a small step is a slider-friendly float. A range
    /// too wide to slide is a number box with steppers and no slider.
    Num {
        min: f64,
        max: f64,
        step: f64,
    },
    /// A hex string.
    Color,
    Text,
    Choice {
        options: &'static [&'static str],
    },
    /// The one structural control: the list of rings, each a scale plus the
    /// shared field list.
    Rings {
        scales: &'static [&'static str],
    },
}

impl Field {
    const fn flag(key: &'static str, label: &'static str) -> Self {
        Self::plain(key, label, Kind::Bool)
    }
    const fn num(key: &'static str, label: &'static str, min: f64, max: f64, step: f64) -> Self {
        Self::plain(key, label, Kind::Num { min, max, step })
    }
    const fn color(key: &'static str, label: &'static str) -> Self {
        Self::plain(key, label, Kind::Color)
    }
    const fn text(key: &'static str, label: &'static str) -> Self {
        Self::plain(key, label, Kind::Text)
    }
    const fn choice(
        key: &'static str,
        label: &'static str,
        options: &'static [&'static str],
    ) -> Self {
        Self::plain(key, label, Kind::Choice { options })
    }
    const fn plain(key: &'static str, label: &'static str, kind: Kind) -> Self {
        Self {
            key,
            label,
            optional: false,
            kind,
        }
    }
    /// `null` means the renderer picks, so the control gets an "auto" checkbox.
    const fn auto(mut self) -> Self {
        self.optional = true;
        self
    }
}

fn is_off(b: &bool) -> bool {
    !*b
}

/// Shared by every ring: `radius` pins a ring instead of letting `layout()`
/// place it, and both angles and thickness are fractions of the half-canvas or
/// degrees off twelve o'clock.
const RADIUS: Field = Field::num("radius", "radius", 0.0, 1.0, 0.005).auto();
const THICKNESS: Field = Field::num("thickness", "thickness", 0.002, 0.2, 0.001);
const OFFSET: Field = Field::num("offset_deg", "offset °", -180.0, 180.0, 0.5);
const REVERSED: Field = Field::flag("reversed", "flip direction");
const SCALE: Field = Field::choice("scale", "scale", &["hour24", "week", "year"]);

/// Every control a ring has, in the order the card shows them. Layer keys are
/// dotted into the ring's `layers` object.
const RING_FIELDS: &[Field] = &[
    RADIUS,
    THICKNESS,
    OFFSET,
    REVERSED,
    SCALE,
    Field::flag("layers.ticks", "ticks"),
    Field::num("layers.major_every", "long tick every h", 1.0, 24.0, 1.0),
    Field::flag("layers.tick_labels", "labels"),
    Field::flag("layers.label_major_only", "labels: major only"),
    Field::flag("layers.hand", "hand"),
    Field::num("layers.hand_inner", "hand inner", 0.0, 1.0, 0.01),
    Field::flag("layers.hand_dot", "hand dot"),
    Field::flag("layers.sun", "sunrise and sunset"),
    Field::flag("layers.night", "night sector"),
    Field::flag("layers.day", "day sector"),
    Field::flag("layers.day_sectors", "day sectors"),
    Field::flag("layers.month_sectors", "month sectors"),
    Field::flag("layers.day_borders", "day borders"),
    Field::flag("layers.weekends", "weekends in red"),
    Field::flag("layers.day_labels", "date numbers"),
    Field::flag("layers.month_ticks", "month ticks"),
    Field::flag("layers.month_labels", "month labels"),
    Field::flag("layers.seasons", "seasons"),
    Field::flag("layers.week_ticks", "week ticks"),
    Field::flag("layers.year_events", "equinoxes and solstices"),
    Field::flag("layers.events", "events"),
    Field::choice("layers.event_style", "event style", &["sector", "notch"]),
    Field::flag("layers.event_labels", "event labels"),
    Field::flag("layers.progress", "progress"),
    Field::flag("layers.today_marker", "today marker"),
];

const SCALES: &[&str] = &["hour24", "week", "month", "year"];

/// The whole schema, grouped for a panel that folds sections shut.
pub fn form() -> Form {
    Form {
        palettes: [Theme::Dark, Theme::Light]
            .into_iter()
            .map(|t| (t.name(), Palette::resolve(t, &Overrides::default())))
            .collect(),
        ring_fields: RING_FIELDS,
        sections: vec![
            Section {
                id: "layout",
                label: "Layout",
                fields: vec![
                    Field::num("spacing", "spacing", 0.0, 1.0, 0.01),
                    Field::num("inner_radius", "inner radius", 0.0, 1.0, 0.005),
                    Field::num("outer_radius", "outer radius", 0.0, 1.0, 0.005),
                    Field {
                        key: "rings",
                        label: "Rings",
                        optional: false,
                        kind: Kind::Rings { scales: SCALES },
                    },
                ],
            },
            Section {
                id: "colours",
                label: "Colours",
                fields: vec![
                    Field::choice("theme", "theme", &["dark", "light"]),
                    Field::color("background", "canvas").auto(),
                    Field::color("colors.background", "background").auto(),
                    Field::color("colors.foreground", "foreground").auto(),
                    Field::color("colors.muted", "muted").auto(),
                    Field::color("colors.accent", "accent").auto(),
                    Field::color("colors.solar", "solar").auto(),
                    Field::color("colors.weekend", "weekend").auto(),
                    Field::color("colors.week_marks", "week marks").auto(),
                    Field::color("colors.border", "border").auto(),
                    Field::color("colors.day", "day").auto(),
                    Field::color("colors.night", "night").auto(),
                    Field::color("colors.seasons.0", "winter").auto(),
                    Field::color("colors.seasons.1", "spring").auto(),
                    Field::color("colors.seasons.2", "summer").auto(),
                    Field::color("colors.seasons.3", "autumn").auto(),
                ],
            },
            Section {
                id: "place",
                label: "Place",
                fields: vec![
                    Field::num("location.lat", "latitude °", -90.0, 90.0, 0.1).auto(),
                    Field::num("location.lon", "longitude °", -180.0, 180.0, 0.1).auto(),
                ],
            },
            Section {
                id: "type",
                label: "Type",
                fields: vec![
                    Field::text("font.family", "family"),
                    Field::num("font.size", "size", 0.002, 0.08, 0.001),
                    Field::num("font.weight", "weight", 100.0, 900.0, 100.0),
                ],
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, Location, Overrides, Ring, Scale};

    /// Every dotted path must reach a key that `Config` actually serialises, or
    /// the control is bound to nothing.
    fn has(value: &serde_json::Value, path: &str) -> bool {
        let mut value = value;
        for part in path.split('.') {
            value = match part
                .parse::<usize>()
                .map_or_else(|_| value.get(part), |i| value.get(i))
            {
                Some(next) => next,
                None => return false,
            };
        }
        true
    }

    fn ring(scale: &str) -> serde_json::Value {
        let scale = match scale {
            "hour24" => Scale::Hour24,
            "week" => Scale::Week,
            _ => Scale::Year,
        };
        serde_json::to_value(Ring::new(scale)).unwrap()
    }

    #[test]
    fn every_key_is_a_real_config_key() {
        // Optional paths only exist in the serialised form once something is
        // behind them, so give the config a location and a full palette.
        let cfg = Config {
            location: Some(Location { lat: 0.0, lon: 0.0 }),
            colors: Overrides {
                seasons: Some([String::new(), String::new(), String::new(), String::new()]),
                ..Default::default()
            },
            ..Default::default()
        };
        let cfg = serde_json::to_value(cfg).unwrap();

        for section in form().sections {
            for field in &section.fields {
                if let Kind::Rings { scales } = &field.kind {
                    for scale in *scales {
                        let value = ring(scale);
                        for ring_field in form().ring_fields {
                            assert!(
                                has(&value, ring_field.key),
                                "ring {scale}: no such config key {:?}",
                                ring_field.key
                            );
                        }
                    }
                } else {
                    assert!(
                        has(&cfg, field.key),
                        "section {}: no such config key {:?}",
                        section.id,
                        field.key
                    );
                }
            }
        }
    }

    /// A number outside its own control's range is a control that cannot reach
    /// its own default.
    #[test]
    fn every_range_covers_the_default() {
        let check = |fields: &[Field], value: &serde_json::Value, what: &str| {
            for field in fields {
                let Kind::Num { min, max, .. } = field.kind else {
                    continue;
                };
                if let Some(n) = value.get(field.key).and_then(serde_json::Value::as_f64) {
                    assert!(
                        n >= min && n <= max,
                        "{what}.{} default {n} is outside {min}..{max}",
                        field.key
                    );
                }
            }
        };
        let cfg = Config::default();
        let value = serde_json::to_value(&cfg).unwrap();
        for section in form().sections {
            for field in &section.fields {
                if let Kind::Rings { scales } = &field.kind {
                    for scale in *scales {
                        let value = ring(scale);
                        check(form().ring_fields, &value, scale);
                    }
                } else {
                    check(std::slice::from_ref(field), &value, section.id);
                }
            }
        }
    }
}
