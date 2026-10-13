//! wycca-core: config + timestamp -> SVG. Pure on purpose: no I/O, no clock, no
//! platform, so the same code serves the CLI, the browser (wasm) and later the
//! Android widget.

mod config;
pub mod events;
mod ffi;
mod geom;
mod rings;
pub mod sun;
mod svg;
pub mod ui;

pub use config::{
    Config, Font, HourRing, IntervalRing, Location, Marker, MonthRing, Overrides, Palette,
    RingSpec, Span, Theme, Unit, WeekRing, YearRing,
};
pub use rings::{Band, Ctx};
pub use svg::Svg;

use chrono::DateTime;

/// The only place allowed to ask the system what time it is. wasm has no clock
/// to ask — the caller passes `t_ms` in, like the CLI does.
#[cfg(not(target_arch = "wasm32"))]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Where each ring sits, inner to outer.
///
/// A ring that pins its own `radius` keeps it. The rest are spread over
/// `[inner_radius, outer_radius]`, with the single `spacing` slider bending the
/// curve: 0 spaces them evenly, 1 squeezes the inner rings together and pushes the
/// outer ones out.
pub fn layout(cfg: &Config) -> Vec<Band> {
    let half = cfg.size as f64 / 2.0;
    let (lo, hi) = (cfg.inner_radius, cfg.outer_radius);
    let n = cfg.rings.len().max(1) as f64;
    cfg.rings
        .iter()
        .enumerate()
        .map(|(i, ring)| {
            let t = ((i + 1) as f64 / n).powf(1.0 + 2.0 * cfg.spacing);
            let outer = ring.radius().unwrap_or(lo + (hi - lo) * t) * half;
            Band {
                r_in: (outer / half - ring.thickness()) * half,
                r_out: outer,
            }
        })
        .collect()
}

/// Render at `t_ms`, a wall-clock timestamp in milliseconds. Same input, same bytes.
pub fn render(cfg: &Config, t_ms: u64) -> String {
    let palette = Palette::resolve(cfg.theme, &cfg.colors);
    let background = cfg.background.as_deref().unwrap_or(&palette.background);
    let mut svg = Svg::new(cfg, background);
    let half = cfg.size as f64 / 2.0;
    let ctx = Ctx {
        palette: &palette,
        now: DateTime::from_timestamp_millis(t_ms as i64).unwrap_or(DateTime::UNIX_EPOCH),
        t_ms,
        cx: half,
        cy: half,
        half,
        font_size: cfg.font.size * cfg.size as f64,
        utc_offset_ms: cfg.utc_offset_ms.unwrap_or(0),
        location: cfg.location,
        events: &cfg.events,
    };
    for (ring, band) in cfg.rings.iter().zip(layout(cfg)) {
        rings::draw(&mut svg, ring, &band, &ctx);
    }
    svg.finish()
}

pub fn parse(json: &str) -> Result<Config, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Location;

    #[test]
    fn rendering_is_a_pure_function_of_config_and_time() {
        let cfg = Config::default();
        assert_eq!(
            render(&cfg, 1_800_000_000_000),
            render(&cfg, 1_800_000_000_000)
        );
    }

    #[test]
    fn the_default_preset_draws_something() {
        let svg = render(&Config::default(), 1_800_000_000_000);
        assert!(svg.contains("<circle") && svg.contains("<path") && svg.contains("<text"));
    }

    #[test]
    fn background_none_is_transparent() {
        let cfg: Config = serde_json::from_str(r#"{"background":"none"}"#).unwrap();
        assert!(!render(&cfg, 0).contains("<rect"));
    }

    /// What the browser keeps in local storage is the serialised config, so the
    /// round trip through JSON has to be lossless or a saved setting dies quietly.
    #[test]
    fn a_config_survives_the_json_round_trip_the_page_stores() {
        let cfg = Config {
            location: Some(Location {
                lat: 51.0,
                lon: 71.4,
            }),
            utc_offset_ms: Some(18_000_000),
            ..Default::default()
        };
        let bytes = serde_json::to_string(&cfg).unwrap();
        let back = parse(&bytes).unwrap();
        assert_eq!(
            render(&back, 1_800_000_000_000),
            render(&cfg, 1_800_000_000_000)
        );
    }

    /// `reversed` has to reach every angle of a ring, not most of them.
    #[test]
    fn reversing_a_ring_changes_what_it_draws() {
        let t = 1_800_000_000_000;
        for ring in ["month", "week", "year"] {
            let plain: Config =
                serde_json::from_str(&format!(r#"{{"rings":[{{"type":"{ring}"}}]}}"#)).unwrap();
            let flipped: Config = serde_json::from_str(&format!(
                r#"{{"rings":[{{"type":"{ring}","reversed":true}}]}}"#
            ))
            .unwrap();
            assert_ne!(render(&plain, t), render(&flipped, t), "{ring}");
        }
    }
}
