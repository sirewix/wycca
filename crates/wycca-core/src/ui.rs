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

use crate::{Layer, Overrides, Palette, Theme};

#[derive(Serialize)]
pub struct Form {
    pub sections: Vec<Section>,
    /// What `Palette::resolve` gives for each theme, so a control that is "auto"
    /// still has a real colour to switch to, and a layer's colour slot still has
    /// a swatch.
    pub palettes: BTreeMap<&'static str, Palette>,
    /// What a ring has besides its layers. A ring card is these, a scale picker,
    /// and the layer list.
    pub ring_fields: &'static [Field],
    /// What a layer has, by the `kind` it was given. Keys are dotted into the
    /// layer object itself, so `disp.border.color` is the border's colour.
    pub layer_fields: BTreeMap<&'static str, Vec<Field>>,
}

#[derive(Serialize)]
pub struct Section {
    pub id: &'static str,
    pub label: &'static str,
    pub fields: Vec<Field>,
}

#[derive(Clone, Serialize)]
pub struct Field {
    /// Dotted path into the config, `"colors.accent"`. A ring or layer field's
    /// path is relative to that one object.
    pub key: &'static str,
    pub label: &'static str,
    /// Null in the config means something other than a value: "let the renderer
    /// decide" for a colour, "off" for a layer's fill. The checkbox beside the
    /// control says which, and `note` is the word for it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<&'static str>,
    #[serde(flatten)]
    pub kind: Kind,
}

/// Everything a control can be. Nothing here knows what a widget looks like.
#[derive(Clone, Serialize)]
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
    /// A hex string, or a palette slot name.
    Color,
    Text,
    /// The name of a feed in `Config::feeds`. The options are not in the form —
    /// they are whatever feeds the config holds — so a UI fills them in.
    Feed,
    Choice {
        options: &'static [&'static str],
    },
    /// The list of rings, each a scale plus the shared field list and its layers.
    Rings {
        scales: &'static [&'static str],
        /// What a layer on such a ring may be.
        kinds: &'static [&'static str],
    },
    /// The list of layers on one ring. Layers are always on show: they are the
    /// ring, and a folded ring is a ring nobody can see.
    Layers {
        kinds: &'static [&'static str],
    },
}

impl Field {
    const fn new(key: &'static str, label: &'static str, kind: Kind) -> Self {
        Self {
            key,
            label,
            optional: None,
            kind,
        }
    }
    /// A control whose config value may be null. `note` is what null means,
    /// written beside the checkbox that puts it back: "auto" for a palette slot
    /// that falls back to the theme, "none" for a part of a layer that is off.
    const fn nullable(
        key: &'static str,
        label: &'static str,
        note: &'static str,
        kind: Kind,
    ) -> Self {
        Self {
            key,
            label,
            optional: Some(note),
            kind,
        }
    }
    const fn flag(key: &'static str, label: &'static str) -> Self {
        Self::new(key, label, Kind::Bool)
    }
    const fn num(key: &'static str, label: &'static str, min: f64, max: f64, step: f64) -> Self {
        Self::new(key, label, Kind::Num { min, max, step })
    }
    const fn text(key: &'static str, label: &'static str) -> Self {
        Self::new(key, label, Kind::Text)
    }
    const fn choice(
        key: &'static str,
        label: &'static str,
        options: &'static [&'static str],
    ) -> Self {
        Self::new(key, label, Kind::Choice { options })
    }
    /// A layer's colour: a palette slot name or a hex, never nothing.
    const fn color(key: &'static str, label: &'static str) -> Self {
        Self::new(key, label, Kind::Color)
    }
    /// A palette slot: null is the theme's own, which is what "auto" gives back.
    const fn auto_color(key: &'static str, label: &'static str) -> Self {
        Self::nullable(key, label, "auto", Kind::Color)
    }
    /// A number that may be left out of the config altogether.
    const fn auto_num(
        key: &'static str,
        label: &'static str,
        min: f64,
        max: f64,
        step: f64,
    ) -> Self {
        Self::nullable(key, label, "auto", Kind::Num { min, max, step })
    }
}

/// Shared by every ring: `radius` pins a ring instead of letting `layout()`
/// place it, and both angles and thickness are fractions of the half-canvas or
/// degrees off twelve o'clock.
const RADIUS: Field = Field::auto_num("radius", "radius", 0.0, 1.0, 0.005);
const THICKNESS: Field = Field::num("thickness", "thickness", 0.002, 0.2, 0.001);
const OFFSET: Field = Field::num("offset_deg", "offset °", -180.0, 180.0, 0.5);
const REVERSED: Field = Field::flag("reversed", "flip direction");
const SCALE: Field = Field::choice("scale", "scale", SCALES);

/// Every control a ring has that is not a layer.
const RING_FIELDS: &[Field] = &[RADIUS, THICKNESS, OFFSET, REVERSED, SCALE];

const SCALES: &[&str] = &["hour24", "week", "month", "year"];
const UNITS: &[&str] = &["year", "month", "week", "day", "hour", "minute"];
const SOLAR: &[&str] = &["day", "night", "sunrise", "sunset", "equinox", "solstice"];
const KINDS: &[&str] = &["free", "busy"];

/// What every layer can draw, whatever its source. Off by default: a layer with
/// nothing set draws nothing, which is the point of two separate things to set.
const DISP: &[Field] = &[
    Field::nullable("disp.background", "fill", "none", Kind::Color),
    Field::nullable("disp.border", "stroke", "none", Kind::Bool),
    Field::choice("disp.border.kind", "stroke as", &["line", "hand"]),
    Field::color("disp.border.color", "stroke colour"),
    Field::num("disp.border.width", "stroke width", 0.0005, 0.05, 0.0005),
    Field::num("disp.border.inner", "hand from", 0.0, 0.5, 0.01),
    Field::nullable("disp.label", "label", "none", Kind::Bool),
    Field::choice("disp.label.at", "label across", &["in", "at", "out"]),
    Field::choice(
        "disp.label.anchor",
        "label along",
        &["start", "middle", "end"],
    ),
    Field::color("disp.label.color", "label colour"),
    Field::num("disp.label.size", "label size", 0.2, 4.0, 0.1),
    Field::nullable("disp.offsets", "offsets", "whole", Kind::Bool),
    Field::num("disp.offsets.inner", "inner %", -100.0, 200.0, 1.0),
    Field::num("disp.offsets.outer", "outer %", -100.0, 200.0, 1.0),
    Field::num("disp.gap", "gap °", 0.0, 10.0, 0.1),
];

/// What a layer's source is, by kind: where its spans come from and how long
/// they are. `amount: 0` is an instant, so a tick is a layer with no length.
fn layer_fields() -> BTreeMap<&'static str, Vec<Field>> {
    [
        (
            "periodic",
            vec![
                Field::choice("unit", "unit", UNITS),
                Field::num("amount", "length", 0.0, 36.0, 1.0),
                Field::num("every", "every", 1.0, 60.0, 1.0),
                // A text box, because a phase is a weekday or a month where the
                // unit has names and a number where it does not.
                Field::text("shift", "from"),
            ],
        ),
        (
            "ics",
            vec![
                Field::new("source", "feed", Kind::Feed),
                Field::choice("only", "only", KINDS),
            ],
        ),
        ("solar", vec![Field::choice("what", "what", SOLAR)]),
        ("now", vec![]),
    ]
    .into_iter()
    .map(|(kind, mut fields)| {
        fields.extend_from_slice(DISP);
        (kind, fields)
    })
    .collect()
}

/// The whole schema, grouped for a panel that folds sections shut.
pub fn form() -> Form {
    Form {
        palettes: [Theme::Dark, Theme::Light]
            .into_iter()
            .map(|t| (t.name(), Palette::resolve(t, &Overrides::default())))
            .collect(),
        ring_fields: RING_FIELDS,
        layer_fields: layer_fields(),
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
                        optional: None,
                        kind: Kind::Rings {
                            scales: SCALES,
                            kinds: Layer::KINDS,
                        },
                    },
                ],
            },
            Section {
                id: "colours",
                label: "Colours",
                fields: vec![
                    Field::choice("theme", "theme", &["dark", "light"]),
                    Field::auto_color("background", "canvas"),
                    Field::auto_color("colors.background", "background"),
                    Field::auto_color("colors.foreground", "foreground"),
                    Field::auto_color("colors.muted", "muted"),
                    Field::auto_color("colors.accent", "accent"),
                    Field::auto_color("colors.solar", "solar"),
                    Field::auto_color("colors.weekend", "weekend"),
                    Field::auto_color("colors.week_marks", "week marks"),
                    Field::auto_color("colors.border", "border"),
                    Field::auto_color("colors.day", "day"),
                    Field::auto_color("colors.night", "night"),
                    Field::auto_color("colors.winter", "winter"),
                    Field::auto_color("colors.spring", "spring"),
                    Field::auto_color("colors.summer", "summer"),
                    Field::auto_color("colors.autumn", "autumn"),
                ],
            },
            Section {
                id: "place",
                label: "Place",
                fields: vec![
                    Field::auto_num("location.lat", "latitude °", -90.0, 90.0, 0.1),
                    Field::auto_num("location.lon", "longitude °", -180.0, 180.0, 0.1),
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
    use crate::{Config, Layer, Location, Overrides, Palette, Ring, Scale};

    /// Every dotted path must reach a key that the config actually serialises, or
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
            "month" => Scale::Month,
            _ => Scale::Year,
        };
        serde_json::to_value(Ring::new(scale)).unwrap()
    }

    /// One layer of `kind` with every part of it spelled out, which is what a
    /// control that reaches into `disp` needs to be checked against.
    fn layer(kind: &str) -> serde_json::Value {
        let source = match kind {
            "periodic" => serde_json::json!({"unit": "day", "amount": 1, "every": 1, "shift": 0}),
            "ics" => serde_json::json!({"source": "holidays", "only": "free"}),
            "solar" => serde_json::json!({"what": "day"}),
            _ => serde_json::json!({}),
        };
        let mut value = serde_json::json!({
            "kind": kind,
            "disp": {
                "background": "#fff",
                "gap": 0.6,
                "border": {"kind": "line", "color": "#fff", "width": 0.004, "inner": 0.1},
                "label": {"at": "in", "anchor": "middle", "color": "#fff", "size": 1.0},
                "offsets": {"inner": 0.0, "outer": 100.0},
            },
        });
        value
            .as_object_mut()
            .unwrap()
            .extend(source.as_object().unwrap().clone());
        value
    }

    #[test]
    fn every_key_is_a_real_config_key() {
        // Optional paths only exist in the serialised form once something is
        // behind them, so give the config a location and a full palette, and
        // every layer the parts it does not start with.
        let full = |s: &str| Some(String::from(s));
        let cfg = Config {
            location: Some(Location { lat: 0.0, lon: 0.0 }),
            colors: Overrides {
                background: full(""),
                foreground: full(""),
                muted: full(""),
                accent: full(""),
                solar: full(""),
                weekend: full(""),
                week_marks: full(""),
                border: full(""),
                day: full(""),
                night: full(""),
                spring: full(""),
                summer: full(""),
                autumn: full(""),
                winter: full(""),
            },
            ..Default::default()
        };
        let cfg = serde_json::to_value(cfg).unwrap();

        for section in form().sections {
            for field in &section.fields {
                if let Kind::Rings { scales, .. } = &field.kind {
                    for scale in *scales {
                        let value = ring(scale);
                        for ring_field in form().ring_fields {
                            assert!(
                                has(&value, ring_field.key),
                                "ring {scale}: no such config key {:?}",
                                ring_field.key
                            );
                        }
                        for layer_kind in Layer::KINDS {
                            let value = layer(layer_kind);
                            for f in &form().layer_fields[*layer_kind] {
                                assert!(
                                    has(&value, f.key),
                                    "layer {layer_kind}: no such config key {:?}",
                                    f.key
                                );
                            }
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

    /// A colour control in a section offers the theme's own colour when "auto" comes
    /// off, and gets it by reading the slot out of its own key. So the last
    /// segment of such a key has to be a slot: `colors.seasons.0` offered a panel
    /// an array `Palette` did not have, and the page died on boot. A layer's own
    /// colour is the exception — its key ends in the part, not the slot — and the
    /// UI knows that from the key.
    #[test]
    fn every_colour_key_ends_in_a_palette_slot() {
        for section in form().sections {
            for field in &section.fields {
                if !matches!(field.kind, Kind::Color) {
                    continue;
                }
                let slot = field.key.split('.').next_back().unwrap();
                assert!(
                    Palette::SLOTS.contains(&slot),
                    "{}: {:?} is not a palette slot",
                    section.id,
                    field.key
                );
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
        let value = serde_json::to_value(Config::default()).unwrap();
        for section in form().sections {
            for field in &section.fields {
                if let Kind::Rings { scales, .. } = &field.kind {
                    for scale in *scales {
                        let value = ring(scale);
                        check(form().ring_fields, &value, scale);
                        for layer_kind in Layer::KINDS {
                            let value = layer(layer_kind);
                            check(&form().layer_fields[*layer_kind], &value, layer_kind);
                        }
                    }
                } else {
                    check(std::slice::from_ref(field), &value, section.id);
                }
            }
        }
    }
}
