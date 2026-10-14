//! One ring, many layers. A ring is a span of time on an annulus — its scale
//! fixes the span, its layers what is drawn on it. A new thing to draw is a new
//! flag in `Layers` plus a block in `draw`; there is no per-ring-type code.
//!
//! Every angle runs through [`ring_angle`], so a layer can never disagree with
//! another about where something sits, and `reversed` is applied once, here,
//! rather than per layer.

use chrono::{DateTime, Datelike, Months, NaiveTime, TimeZone, Utc};

use crate::{
    config::{EventStyle, Location, Palette, Ring, Scale, date},
    events::Kind,
    geom::{polar, rad, radial, sector, turn},
    sun,
    svg::Svg,
};

/// Everything a ring may read. The clock is `now`, injected from outside.
pub struct Ctx<'a> {
    pub palette: &'a Palette,
    pub now: DateTime<Utc>,
    pub t_ms: u64,
    pub cx: f64,
    pub cy: f64,
    pub half: f64,
    pub font_size: f64,
    /// Where the wall clock sits relative to UTC; solar events need it.
    pub utc_offset_ms: i64,
    pub location: Option<Location>,
    pub events: &'a [crate::events::Event],
}

/// A ring's radial extent, resolved once by [`crate::layout`].
pub struct Band {
    pub r_in: f64,
    pub r_out: f64,
}

/// Angular gap between sectors, in degrees, so adjacent sectors read as segments.
const GAP: f64 = 0.6;

/// Width of ticks and markers, as a fraction of the half-canvas.
const MARKER: f64 = 0.004;

pub fn draw(svg: &mut Svg, ring: &Ring, band: &Band, ctx: &Ctx) {
    let (cx, cy, half) = (ctx.cx, ctx.cy, ctx.half);
    let (r_in, r_out) = (band.r_in, band.r_out);
    let (start, end) = ring.scale.period(ctx.now);
    let l = &ring.layers;
    let ang = |t: DateTime<Utc>| ring_angle(ring, start, end, t);

    // Sectors sit under everything else.
    if l.seasons {
        // Season boundaries at the meteorological season starts, so each season
        // holds exactly its three months. Winter (Dec 1 to Mar 1) crosses the
        // period's zero, so its end is unwrapped to keep the arc the short way.
        let y = start.year();
        let bounds = [
            date(y - 1, 12, 1),
            date(y, 3, 1),
            date(y, 6, 1),
            date(y, 9, 1),
        ];
        let span = (end - start).num_milliseconds().max(1) as f64;
        let nat = |t: DateTime<Utc>| {
            rad(ring.offset_deg
                + ring.scale.dir() * (t - start).num_milliseconds() as f64 / span * 360.0)
        };
        for (i, s) in bounds.iter().enumerate() {
            let a0 = nat(*s);
            let mut a1 = nat(bounds[(i + 1) % 4]);
            if ring.scale.dir() < 0.0 && a1 > a0 {
                a1 -= rad(360.0);
            } else if ring.scale.dir() > 0.0 && a1 < a0 {
                a1 += rad(360.0);
            }
            svg.path(
                &sector(
                    cx,
                    cy,
                    r_in,
                    r_out,
                    turn(ring.reversed, a0),
                    turn(ring.reversed, a1),
                ),
                &ctx.palette.seasons[i],
            );
        }
    }

    if l.month_sectors {
        let mut at = start;
        while at < end {
            let next = at
                .checked_add_months(Months::new(1))
                .unwrap_or(end)
                .min(end);
            svg.path(
                &sector(
                    cx,
                    cy,
                    r_in,
                    r_out,
                    ang(at) + rad(GAP / 2.0),
                    ang(next) - rad(GAP / 2.0),
                ),
                &ctx.palette.muted,
            );
            at = next;
        }
    }

    if l.day_sectors {
        for day in 0..7 {
            let color = match day {
                5 | 6 => &ctx.palette.weekend,
                _ => &ctx.palette.muted,
            };
            let (a0, a1) = (
                start + chrono::Duration::days(day),
                start + chrono::Duration::days(day + 1),
            );
            svg.path(
                &sector(
                    cx,
                    cy,
                    r_in,
                    r_out,
                    ang(a0) + rad(GAP / 2.0),
                    ang(a1) - rad(GAP / 2.0),
                ),
                color,
            );
        }
    }

    if l.progress {
        svg.path(
            &sector(cx, cy, r_in, r_out, ang(start), ang(ctx.now)),
            &ctx.palette.foreground,
        );
    }

    if l.weekends || l.day_borders {
        let mut at = start;
        while at < end {
            let is_weekend = matches!(at.weekday().num_days_from_monday(), 5 | 6);
            if l.weekends && is_weekend {
                // A red day sector, the way the week ring marks its weekend.
                let next = at + chrono::Duration::days(1);
                svg.path(
                    &sector(cx, cy, r_in, r_out, ang(at), ang(next)),
                    &ctx.palette.weekend,
                );
            } else if l.day_borders {
                svg.line(
                    &radial(
                        cx,
                        cy,
                        r_in + (r_out - r_in) * 0.55,
                        r_in + (r_out - r_in) * 0.9,
                        ang(at),
                    ),
                    &ctx.palette.border,
                    MARKER * half * 0.6,
                );
            }
            at += chrono::Duration::days(1);
        }
    }

    if l.week_ticks {
        let mut at = start;
        while at.weekday().num_days_from_monday() != 0 {
            at += chrono::Duration::days(1);
        }
        while at < end {
            svg.line(
                &radial(cx, cy, r_in, r_in + (r_out - r_in) * 0.3, ang(at)),
                &ctx.palette.week_marks,
                MARKER * half * 0.6,
            );
            at += chrono::Duration::days(7);
        }
    }

    if l.month_ticks {
        for m in 0..12 {
            let at = start.checked_add_months(Months::new(m)).unwrap_or(start);
            if at >= end {
                break;
            }
            svg.line(
                &radial(cx, cy, r_in - half * 0.01, r_out, ang(at)),
                &ctx.palette.border,
                MARKER * half,
            );
            if l.month_labels {
                let a = ang(at + chrono::Duration::days(15));
                let (x, y) = polar(cx, cy, r_out + half * 0.04, a);
                svg.text(
                    x,
                    y,
                    ctx.font_size,
                    &ctx.palette.foreground,
                    "middle",
                    &at.month().to_string(),
                );
            }
        }
    }

    // Day and night are sectors, so they go down with the other sectors: the
    // ticks land on top of them and stay readable on the lit half.
    if (l.day || l.night)
        && let Some(loc) = ctx.location
        && matches!(ring.scale, Scale::Hour24)
        && let Some((rise, set)) = sun::daylight(ctx.now.date_naive(), loc.lat, loc.lon)
    {
        let a_rise = ang(ctx.wall(rise));
        let a_set = ang(ctx.wall(set));
        if l.day {
            svg.path(
                &sector(cx, cy, r_in, r_out, a_rise, a_set),
                &ctx.palette.day,
            );
        }
        if l.night {
            // Sunset round to sunrise, the long way forward through midnight.
            let a1 = if a_rise > a_set {
                a_rise
            } else {
                a_rise + rad(360.0)
            };
            svg.path(&sector(cx, cy, r_in, r_out, a_set, a1), &ctx.palette.night);
        }
    }

    if l.ticks {
        let major = l.major_every.max(1);
        let w = (ring.thickness * half).max(1.0);
        for h in 0..24 {
            let a = ang(start + chrono::Duration::hours(i64::from(h)));
            // Short ticks for the hours between two majors.
            let (i, o, lw) = match h % major {
                0 => (r_in, r_out, w),
                _ => (r_in + (r_out - r_in) * 0.55, r_out, w * 0.45),
            };
            svg.line(&radial(cx, cy, i, o, a), &ctx.palette.muted, lw);
        }
    }

    if l.sun
        && let Some(loc) = ctx.location
        && let Some((rise, set)) = sun::daylight(ctx.now.date_naive(), loc.lat, loc.lon)
    {
        for t in [rise, set] {
            svg.line(
                &radial(cx, cy, r_in, r_out, ang(ctx.wall(t))),
                &ctx.palette.solar,
                MARKER * half * 0.8,
            );
        }
    }

    if l.year_events {
        // Global instants, slid onto the wall clock by the UTC offset.
        for t in sun::events_between(start, end) {
            svg.line(
                &radial(
                    cx,
                    cy,
                    r_in - half * 0.02,
                    r_out + half * 0.02,
                    ang(ctx.wall(t)),
                ),
                &ctx.palette.solar,
                MARKER * half,
            );
        }
    }

    if l.events && !ctx.events.is_empty() {
        let in_window =
            |e: &crate::events::Event| e.date >= start.date_naive() && e.date <= end.date_naive();
        match l.event_style {
            EventStyle::Sector => {
                for e in ctx.events.iter().filter(|e| in_window(e)) {
                    let at = e.date.and_time(NaiveTime::MIN).and_utc();
                    let next = e
                        .date
                        .succ_opt()
                        .unwrap_or(e.date)
                        .and_time(NaiveTime::MIN)
                        .and_utc();
                    svg.path(
                        &sector(cx, cy, r_in, r_out, ang(at), ang(next)),
                        &ctx.palette.weekend,
                    );
                }
            }
            EventStyle::Notch => {
                let mut placed: Vec<f64> = Vec::new();
                let mid_r = (r_in + r_out) / 2.0;
                for event in ctx.events.iter().filter(|e| in_window(e)) {
                    let at = event.date.and_time(NaiveTime::MIN).and_utc();
                    let a = ang(at);
                    let deg = a.to_degrees().rem_euclid(360.0);
                    let color = match event.kind {
                        Kind::Free => &ctx.palette.weekend,
                        Kind::Busy => &ctx.palette.foreground,
                    };
                    let (i, o, w) = match event.kind {
                        Kind::Free => {
                            (r_in - half * 0.02, r_out + half * 0.02, MARKER * half * 0.8)
                        }
                        Kind::Busy => (r_in, r_in + (r_out - r_in) * 0.45, MARKER * half * 0.6),
                    };
                    svg.line(&radial(cx, cy, i, o, a), color, w);

                    if l.event_labels && event.kind == Kind::Free {
                        // Greedy: skip a label that would land on the previous one.
                        let span = event.label.chars().count() as f64 * ctx.font_size * 0.55
                            / mid_r
                            * 180.0
                            / std::f64::consts::PI;
                        if placed.iter().all(|p| (deg - p).abs() > span + 1.0) {
                            placed.push(deg);
                            let (x, y) = polar(cx, cy, mid_r, a);
                            svg.text(
                                x,
                                y,
                                ctx.font_size * 0.8,
                                &ctx.palette.border,
                                "middle",
                                &event.label,
                            );
                        }
                    }
                }
            }
        }
    }

    if l.tick_labels {
        let major = l.major_every.max(1);
        for h in 0..24 {
            if l.label_major_only && h % major != 0 {
                continue;
            }
            let a = ang(start + chrono::Duration::hours(i64::from(h)));
            // Inside the ring, but never past the centre.
            let (x, y) = polar(cx, cy, (r_in - half * 0.035).max(0.0), a);
            svg.text(
                x,
                y,
                ctx.font_size,
                &ctx.palette.foreground,
                "middle",
                &h.to_string(),
            );
        }
    }

    if l.day_labels {
        let mut at = start;
        while at < end {
            let a = ang(at + chrono::Duration::hours(12));
            let (x, y) = polar(cx, cy, (r_in + r_out) / 2.0, a);
            svg.text(
                x,
                y,
                ctx.font_size * 0.7,
                &ctx.palette.foreground,
                "middle",
                &at.day().to_string(),
            );
            at += chrono::Duration::days(1);
        }
    }

    if l.today_marker {
        svg.line(
            &radial(
                cx,
                cy,
                r_in - half * 0.012,
                r_out + half * 0.012,
                ang(ctx.now),
            ),
            &ctx.palette.accent,
            MARKER * half * 1.5,
        );
    }

    if l.hand {
        svg.line(
            &radial(cx, cy, l.hand_inner * half, r_out, ang(ctx.now)),
            &ctx.palette.accent,
            MARKER * half * 1.5,
        );
        if l.hand_dot {
            svg.circle(cx, cy, half * 0.008, &ctx.palette.accent);
        }
    }
}

/// The angle of `t` on a ring: its fraction of the period, in the scale's
/// direction, rotated by `offset_deg`, flipped once by `reversed`.
fn ring_angle(ring: &Ring, start: DateTime<Utc>, end: DateTime<Utc>, t: DateTime<Utc>) -> f64 {
    let span = (end - start).num_milliseconds().max(1) as f64;
    let frac = ((t - start).num_milliseconds() as f64 / span).clamp(0.0, 1.0);
    turn(
        ring.reversed,
        rad(ring.offset_deg + ring.scale.dir() * frac * 360.0),
    )
}

impl Ctx<'_> {
    /// The same instant, as the wall clock reads it.
    fn wall(&self, t: DateTime<Utc>) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(t.timestamp_millis() + self.utc_offset_ms)
            .single()
            .unwrap_or(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Scale;
    use chrono::NaiveDate;

    fn date(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap_or_default()
            .and_time(NaiveTime::MIN)
            .and_utc()
    }

    fn ring_angle_deg(scale: Scale, t: DateTime<Utc>, reversed: bool) -> f64 {
        let ring = Ring {
            offset_deg: 0.0,
            reversed,
            scale,
            ..Ring::new(scale)
        };
        let (start, end) = scale.period(t);
        ring_angle(&ring, start, end, t).to_degrees()
    }

    #[test]
    fn the_year_starts_at_january_and_runs_backwards() {
        let jan = ring_angle_deg(Scale::Year, date(2027, 1, 1), false);
        let feb15 = ring_angle_deg(Scale::Year, date(2027, 2, 15), false);
        assert!(jan.abs() < 1e-6, "January starts at the top: {jan}");
        assert!(feb15 < jan, "the year must not run clockwise: {feb15}");
    }

    #[test]
    fn each_season_holds_exactly_its_three_months() {
        // Winter is Dec-Feb, spring Mar-May, summer Jun-Aug, autumn Sep-Nov.
        let quadrant = |deg: f64| {
            let d = deg.rem_euclid(360.0);
            if d < 45.0 || d >= 315.0 {
                0
            } else if d < 135.0 {
                3
            } else if d < 225.0 {
                2
            } else {
                1
            }
        };
        let y26 = [
            (12, date(2026, 12, 1)),
            (1, date(2027, 1, 1)),
            (2, date(2027, 2, 1)),
        ];
        let y27 = [
            (3, date(2027, 3, 1)),
            (4, date(2027, 4, 1)),
            (5, date(2027, 5, 1)),
        ];
        let y28 = [
            (6, date(2027, 6, 1)),
            (7, date(2027, 7, 1)),
            (8, date(2027, 8, 1)),
        ];
        let y29 = [
            (9, date(2027, 9, 1)),
            (10, date(2027, 10, 1)),
            (11, date(2027, 11, 1)),
        ];
        for (months, q) in [(y26, 0), (y27, 1), (y28, 2), (y29, 3)] {
            for (m, at) in months {
                let deg = ring_angle_deg(Scale::Year, at, false);
                assert_eq!(
                    quadrant(deg),
                    q,
                    "month {m} starts in the wrong season: {deg} deg"
                );
            }
        }
    }

    #[test]
    fn season_boundaries_land_on_the_month_borders() {
        // Winter starts on Dec 1, so its border is a month border; the same must
        // hold for Mar 1, Jun 1 and Sep 1. This is the `nat` formula `draw` uses.
        let start = date(2027, 1, 1);
        let end = date(2028, 1, 1);
        let span = (end - start).num_milliseconds().max(1) as f64;
        let nat = |t: DateTime<Utc>| {
            Scale::Year.dir() * (t - start).num_milliseconds() as f64 / span * 360.0
        };
        for m in [12, 3, 6, 9] {
            let at = if m == 12 {
                date(2026, 12, 1)
            } else {
                date(2027, m, 1)
            };
            // The season border and the month tick come from the same angle of
            // the same instant, so they cannot drift apart (mod 360: the
            // December border sits on the other side of the turn).
            let tick = ring_angle_deg(Scale::Year, at, false);
            let border = nat(at);
            assert!(
                (tick - border)
                    .rem_euclid(360.0)
                    .min((border - tick).rem_euclid(360.0))
                    < 1e-6,
                "season {m} border {border} and month tick {tick} disagree"
            );
        }
    }

    #[test]
    fn the_year_runs_counter_clockwise() {
        let spring = ring_angle_deg(Scale::Year, date(2027, 3, 20), false);
        let summer = ring_angle_deg(Scale::Year, date(2027, 6, 20), false);
        assert!(
            spring > summer,
            "time must not advance clockwise: {spring} -> {summer}"
        );
    }

    #[test]
    fn the_week_starts_on_monday_and_runs_clockwise() {
        let mon = ring_angle_deg(Scale::Week, date(2026, 10, 12), false);
        let sun = ring_angle_deg(Scale::Week, date(2026, 10, 18), false);
        assert!(mon.abs() < 1e-6, "Monday starts at the top: {mon}");
        assert!((300.0..360.0).contains(&sun), "Sunday is 6/7 round: {sun}");
    }

    #[test]
    fn an_hour_is_fifteen_degrees_on_the_day_scale() {
        let midnight = ring_angle_deg(Scale::Hour24, date(2026, 10, 14), false);
        let six = ring_angle_deg(
            Scale::Hour24,
            date(2026, 10, 14) + chrono::Duration::hours(6),
            false,
        );
        let eighteen = ring_angle_deg(
            Scale::Hour24,
            date(2026, 10, 14) + chrono::Duration::hours(18),
            false,
        );
        assert!(midnight.abs() < 1e-6);
        assert!((six - 90.0).abs() < 1e-6, "06:00 is a quarter turn: {six}");
        assert!(
            (eighteen - 270.0).abs() < 1e-6,
            "18:00 is three quarters: {eighteen}"
        );
    }

    #[test]
    fn reversing_a_ring_runs_it_the_other_way_round() {
        let t = date(2027, 1, 31);
        for scale in [Scale::Hour24, Scale::Week, Scale::Year] {
            let plain = ring_angle_deg(scale, t, false);
            let flipped = ring_angle_deg(scale, t, true);
            assert!(
                (plain + flipped).abs() < 1e-6,
                "{scale:?}: reversed must negate: {plain} vs {flipped}"
            );
        }
    }

    #[test]
    fn months_are_as_wide_as_they_are_long() {
        // On the year scale a month's width in degrees is its share of the year.
        let width = |year: i32, month: u32| {
            let start = date(year, month, 1);
            let end = if month == 12 {
                date(year + 1, 1, 1)
            } else {
                date(year, month + 1, 1)
            };
            let a0 = ring_angle_deg(Scale::Year, start, false);
            let a1 = ring_angle_deg(Scale::Year, end, false);
            (a0 - a1).abs()
        };
        // 2026 is not a leap year, so February is 28 days.
        assert!((width(2026, 2) - 28.0 / 365.0 * 360.0).abs() < 0.1);
        assert!(
            width(2026, 7) > width(2026, 2),
            "July must be wider than February"
        );
    }

    #[test]
    fn the_year_period_covers_the_calendar_year() {
        let (start, end) = Scale::Year.period(date(2026, 6, 15));
        assert_eq!((start.year(), start.month(), start.day()), (2026, 1, 1));
        assert_eq!((end.year(), end.month(), end.day()), (2027, 1, 1));
    }
}
