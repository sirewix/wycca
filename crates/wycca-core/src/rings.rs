//! One function per ring type. A new ring is a new `RingSpec` variant plus a `match`
//! arm here; there is no plugin hook, and there should not be one until a second
//! consumer (the widget) needs to add rings without touching this file.

use chrono::{DateTime, Datelike, Months, NaiveDate, NaiveTime, TimeZone, Timelike, Utc};

use crate::{
    config::{HourRing, IntervalRing, Location, MonthRing, Palette, RingSpec, WeekRing, YearRing},
    events::Kind,
    geom::{polar, rad, radial, sector},
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

/// Angular gap between sectors, in degrees, so adjacent rings read as segments.
const GAP: f64 = 0.6;

/// Width of ticks and markers, as a fraction of the half-canvas. Hour ticks use
/// their own ring thickness instead; this covers everything else.
const MARKER: f64 = 0.004;

pub fn draw(svg: &mut Svg, ring: &RingSpec, band: &Band, ctx: &Ctx) {
    match ring {
        RingSpec::Hour24(r) => hour24(svg, r, band, ctx),
        RingSpec::Week(r) => week(svg, r, band, ctx),
        RingSpec::Month(r) => month(svg, r, band, ctx),
        RingSpec::Year(r) => year(svg, r, band, ctx),
        RingSpec::Interval(r) => interval(svg, r, band, ctx),
    }
}

impl Ctx<'_> {
    /// The same instant, as the wall clock reads it.
    fn wall(&self, t: DateTime<Utc>) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(t.timestamp_millis() + self.utc_offset_ms)
            .single()
            .unwrap_or(t)
    }
}

/// Hours into the current day, fractional.
fn hours_into_day(now: DateTime<Utc>) -> f64 {
    now.hour() as f64 + (now.minute() as f64 + now.second() as f64 / 60.0) / 60.0
}

fn hour24(svg: &mut Svg, r: &HourRing, band: &Band, ctx: &Ctx) {
    let (cx, cy, half) = (ctx.cx, ctx.cy, ctx.half);
    let (r_in, r_out) = (band.r_in, band.r_out);
    let major = r.major_every.max(1) as usize;

    if r.ticks {
        let w = (r.thickness * half).max(1.0);
        for h in 0..24 {
            let a = rad(r.offset_deg + h as f64 * 15.0);
            // Short ticks for the hours between two majors.
            let (i, o, lw) = match h % major {
                0 => (r_in, r_out, w),
                _ => (r_in + (r_out - r_in) * 0.55, r_out, w * 0.45),
            };
            svg.line(&radial(cx, cy, i, o, a), &ctx.palette.muted, lw);
        }
    }

    if r.labels {
        for h in 0..24 {
            if r.label_major_only && h % major != 0 {
                continue;
            }
            let a = rad(r.offset_deg + h as f64 * 15.0);
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

    if r.hand {
        let a = rad(r.offset_deg + hours_into_day(ctx.now) * 15.0);
        svg.line(
            &radial(cx, cy, r.hand_inner * half, r_out, a),
            &ctx.palette.accent,
            MARKER * half * 1.5,
        );
        if r.hand_dot {
            svg.circle(cx, cy, half * 0.008, &ctx.palette.accent);
        }
    }

    // Sunrise and sunset, in daylight colour so they never read as "now".
    if r.sun
        && let Some(loc) = ctx.location
        && let Some((rise, set)) = sun::daylight(ctx.now.date_naive(), loc.lat, loc.lon)
    {
        for t in [rise, set] {
            let a = rad(r.offset_deg + hours_into_day(ctx.wall(t)) * 15.0);
            svg.line(
                &radial(cx, cy, r_in, r_out, a),
                &ctx.palette.solar,
                MARKER * half * 0.8,
            );
        }
    }
}

fn week(svg: &mut Svg, r: &WeekRing, band: &Band, ctx: &Ctx) {
    let (cx, cy, half) = (ctx.cx, ctx.cy, ctx.half);
    let (r_in, r_out) = (band.r_in, band.r_out);
    let today = ctx.now.weekday().num_days_from_monday();
    // Monday at the top, or today at the top.
    let offset = r.offset_deg
        + if r.monday_at_top {
            0.0
        } else {
            -(today as f64 * STEP)
        };
    let step = rad(STEP);

    for day in 0..7 {
        let color = match day {
            5 | 6 => &ctx.palette.weekend,
            _ => &ctx.palette.muted,
        };
        svg.path(
            &sector(
                cx,
                cy,
                r_in,
                r_out,
                offset + day as f64 * step + rad(GAP / 2.0),
                offset + (day + 1) as f64 * step - rad(GAP / 2.0),
            ),
            color,
        );
    }

    if r.today_marker {
        // Swept through the day rather than parked on its start.
        let a = offset + (today as f64 + hours_into_day(ctx.now) / 24.0) * step;
        svg.line(
            &radial(cx, cy, r_in - half * 0.012, r_out + half * 0.012, a),
            &ctx.palette.accent,
            MARKER * half * 1.5,
        );
    }
}

/// Days in the year `y`, 365 or 366.
fn days_in_year(y: i32) -> i64 {
    if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
        366
    } else {
        365
    }
}

/// Angle of a moment within its year, in degrees. January starts at the top and
/// the year runs counter-clockwise, the same way the year ring runs.
fn year_progress(now: DateTime<Utc>) -> f64 {
    let elapsed = now.ordinal() as f64 - 1.0 + hours_into_day(now) / 24.0;
    -elapsed / days_in_year(now.year()) as f64 * 360.0
}

fn month(svg: &mut Svg, r: &MonthRing, band: &Band, ctx: &Ctx) {
    let (cx, cy, half) = (ctx.cx, ctx.cy, ctx.half);
    let (r_in, r_out) = (band.r_in, band.r_out);
    let year = ctx.now.year();
    let total = days_in_year(year);
    let angle = |day: i64| rad(r.offset_deg - day as f64 / total as f64 * 360.0);

    // Months are as wide as they are long.
    let mut day = 0;
    for m in 1..=12 {
        let a0 = angle(day);
        let a1 = angle(day + days_of_month(year, m));
        svg.path(
            &sector(
                cx,
                cy,
                r_in,
                r_out,
                a0 + rad(GAP / 2.0),
                a1 - rad(GAP / 2.0),
            ),
            &ctx.palette.muted,
        );
        day += days_of_month(year, m);
    }

    let now = rad(r.offset_deg + year_progress(ctx.now));
    if r.progress {
        // From the first of this month, not from January.
        let month_start = ctx.now.ordinal() as i64 - ctx.now.day() as i64 + 1;
        let start = rad(r.offset_deg - month_start as f64 / total as f64 * 360.0);
        svg.path(
            &sector(cx, cy, r_in, r_out, start, now),
            &ctx.palette.foreground,
        );
    }
    if r.today_marker {
        svg.line(
            &radial(cx, cy, r_in - half * 0.012, r_out + half * 0.012, now),
            &ctx.palette.accent,
            MARKER * half * 1.5,
        );
    }
}

fn days_of_month(year: i32, month: u32) -> i64 {
    // Day 0 of the next month is the same length, and `date` is already total.
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .unwrap_or_default();
    (next - NaiveDate::from_ymd_opt(year, month, 1).unwrap_or_default()).num_days()
}

/// Degrees per weekday.
const STEP: f64 = 360.0 / 7.0;

/// Northern hemisphere, Dec-Feb first.
const MONTH_SEASON: [usize; 12] = [0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 0];

/// Where each season's quarter starts, clockwise from 12 o'clock. The seasons sit
/// symmetric about the cardinal axes (winter top, spring left, summer bottom,
/// autumn right), so the year starts mid-winter rather than on a quadrant edge.
const QUADRANT_START: [f64; 4] = [45.0, -45.0, -135.0, -225.0];

fn date(y: i32, m: u32, d: u32) -> DateTime<Utc> {
    NaiveDate::from_ymd_opt(y, m, d)
        .unwrap_or_default()
        .and_time(NaiveTime::MIN)
        .and_utc()
}

/// First day of the season `dt` falls in.
fn season_start(dt: DateTime<Utc>) -> DateTime<Utc> {
    let (y, m) = (dt.year(), dt.month());
    match MONTH_SEASON[(m - 1) as usize] {
        0 => date(if m <= 2 { y - 1 } else { y }, 12, 1),
        1 => date(y, 3, 1),
        2 => date(y, 6, 1),
        _ => date(y, 9, 1),
    }
}

/// Angle of an instant within its season, in radians. Counter-clockwise, so negative.
fn year_angle(dt: DateTime<Utc>) -> f64 {
    let start = season_start(dt);
    let end = start.checked_add_months(Months::new(3)).unwrap_or(start);
    let span = (end - start).num_milliseconds().max(1) as f64;
    let frac = ((dt - start).num_milliseconds() as f64 / span).clamp(0.0, 1.0);
    rad(QUADRANT_START[MONTH_SEASON[(dt.month() - 1) as usize]] - 90.0 * frac)
}

/// The Dec 1 -> Dec 1 turn that contains `dt`.
fn cycle(dt: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let mut start = season_start(dt);
    while start.month() != 12 {
        start = start.checked_sub_months(Months::new(1)).unwrap_or(start);
    }
    (
        start,
        start.checked_add_months(Months::new(12)).unwrap_or(start),
    )
}

fn year(svg: &mut Svg, r: &YearRing, band: &Band, ctx: &Ctx) {
    let (cx, cy, half) = (ctx.cx, ctx.cy, ctx.half);
    let (r_in, r_out) = (band.r_in, band.r_out);

    for (i, start) in QUADRANT_START.iter().enumerate() {
        svg.path(
            &sector(
                cx,
                cy,
                r_in,
                r_out,
                rad(r.offset_deg + start),
                rad(r.offset_deg + start - 90.0),
            ),
            &ctx.palette.seasons[i],
        );
    }

    let (cycle_start, cycle_end) = cycle(ctx.now);

    if r.week_ticks {
        // Walk back to the Monday on or before the cycle start, so ticks are real weeks.
        let mut at = cycle_start;
        while at.weekday().num_days_from_monday() != 0 {
            at -= chrono::Duration::days(1);
        }
        while at < cycle_end {
            let a = rad(r.offset_deg) + year_angle(at);
            svg.line(
                &radial(cx, cy, r_in, r_in + (r_out - r_in) * 0.3, a),
                &ctx.palette.week_marks,
                MARKER * half * 0.6,
            );
            at += chrono::Duration::days(7);
        }
    }

    if r.month_ticks {
        for m in 0..12 {
            let start = cycle_start
                .checked_add_months(Months::new(m))
                .unwrap_or(cycle_start);
            let a = rad(r.offset_deg) + year_angle(start);
            svg.line(
                &radial(cx, cy, r_in - half * 0.01, r_out, a),
                &ctx.palette.border,
                MARKER * half,
            );
            if r.month_labels {
                let mid = start + chrono::Duration::days(15);
                let a = rad(r.offset_deg) + year_angle(mid);
                let (x, y) = polar(cx, cy, r_out + half * 0.04, a);
                svg.text(
                    x,
                    y,
                    ctx.font_size,
                    &ctx.palette.foreground,
                    "middle",
                    &(m + 1).to_string(),
                );
            }
        }
    }

    if r.year_events {
        // Global instants, slid onto the wall clock by the UTC offset.
        for t in sun::events_between(cycle_start, cycle_end) {
            let a = rad(r.offset_deg) + year_angle(ctx.wall(t));
            svg.line(
                &radial(cx, cy, r_in - half * 0.02, r_out + half * 0.02, a),
                &ctx.palette.solar,
                MARKER * half,
            );
        }
    }

    if r.events && !ctx.events.is_empty() {
        // A day off gets the full crossing line in the weekend red; a merely busy
        // day gets a short notch, so the two never look the same.
        let mut placed: Vec<f64> = Vec::new();
        let mid_r = (r_in + r_out) / 2.0;
        for event in ctx
            .events
            .iter()
            .filter(|e| e.date >= cycle_start.date_naive() && e.date <= cycle_end.date_naive())
        {
            let at = event.date.and_time(NaiveTime::MIN).and_utc();
            let a = rad(r.offset_deg) + year_angle(at);
            let deg = a.to_degrees().rem_euclid(360.0);
            let color = match event.kind {
                Kind::Free => &ctx.palette.weekend,
                Kind::Busy => &ctx.palette.foreground,
            };
            let (i, o, w) = match event.kind {
                Kind::Free => (r_in - half * 0.02, r_out + half * 0.02, MARKER * half * 0.8),
                Kind::Busy => (r_in, r_in + (r_out - r_in) * 0.45, MARKER * half * 0.6),
            };
            svg.line(&radial(cx, cy, i, o, a), color, w);

            if r.event_labels && event.kind == Kind::Free {
                // Greedy: skip a label that would land on the previous one.
                let span = event.label.chars().count() as f64 * ctx.font_size * 0.55 / mid_r
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

    if r.today_marker {
        let a = rad(r.offset_deg) + year_angle(ctx.now);
        svg.line(
            &radial(cx, cy, r_in - half * 0.02, r_out + half * 0.02, a),
            &ctx.palette.accent,
            MARKER * half * 1.5,
        );
    }
}

/// The generic n-times ring: sprints, release trains, months once new moons feed it.
fn interval(svg: &mut Svg, r: &IntervalRing, band: &Band, ctx: &Ctx) {
    let (cx, cy, half) = (ctx.cx, ctx.cy, ctx.half);
    let (r_in, r_out) = (band.r_in, band.r_out);
    let period = (r.every.count * r.every.unit.millis()).max(1) as i64;
    let t = ctx.t_ms as i64 - r.phase_ms;
    let start = t.div_euclid(period) * period + r.phase_ms;
    let frac = (t - start) as f64 / period as f64;
    let offset = rad(r.offset_deg);

    if r.progress {
        svg.path(
            &sector(cx, cy, r_in, r_out, offset, offset + rad(360.0 * frac)),
            &ctx.palette.accent,
        );
    }

    for m in &r.markers {
        let frac = ((m.at_ms - start) as f64 / period as f64).rem_euclid(1.0);
        let a = offset + rad(360.0 * frac);
        svg.line(
            &radial(cx, cy, r_in - half * 0.01, r_out + half * 0.01, a),
            &ctx.palette.foreground,
            MARKER * half,
        );
        if r.labels
            && let Some(label) = &m.label
        {
            let (x, y) = polar(cx, cy, r_out + half * 0.04, a);
            svg.text(
                x,
                y,
                ctx.font_size,
                &ctx.palette.foreground,
                "middle",
                label,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deg(dt: DateTime<Utc>) -> f64 {
        year_angle(dt).to_degrees()
    }

    #[test]
    fn months_are_as_wide_as_they_are_long() {
        let year = 2026;
        let total = days_in_year(year) as f64;
        let width = |m: u32| days_of_month(year, m) as f64 / total * 360.0;
        assert!((width(1) - 31.0 / 365.0 * 360.0).abs() < 0.01);
        // 2026 is not a leap year, so February is 28 days.
        assert!((width(2) - 28.0 / 365.0 * 360.0).abs() < 0.01);
        assert!(width(7) > width(2), "July must be wider than February");
        let all: i64 = (1..=12).map(|m| days_of_month(year, m)).sum();
        assert_eq!(all, days_in_year(year));
    }

    #[test]
    fn the_month_ring_starts_at_the_top_and_runs_backwards() {
        let jan = year_progress(date(2026, 1, 1));
        let june = year_progress(date(2026, 6, 15));
        assert!(jan.abs() < 1e-6, "January starts at the top: {jan}");
        assert!(june < jan, "the year must not run clockwise: {june}");
    }

    #[test]
    fn seasons_are_symmetric_about_the_cardinal_axes() {
        let dec = date(2026, 12, 1);
        let mar = date(2026, 3, 1);
        let jun = date(2026, 6, 1);
        let sep = date(2026, 9, 1);
        // Quarter starts, clockwise from the top.
        assert!((deg(dec) - 45.0).abs() < 1e-6);
        assert!((deg(mar) + 45.0).abs() < 1e-6);
        assert!((deg(jun) + 135.0).abs() < 1e-6);
        assert!((deg(sep) + 225.0).abs() < 1e-6);
    }

    #[test]
    fn the_year_does_not_start_on_a_quarter_edge() {
        let jan = deg(date(2026, 1, 1));
        assert!(
            (jan - 45.0).abs() > 1.0,
            "Jan 1 landed on a quadrant edge: {jan}"
        );
        assert!(
            (0.0..45.0).contains(&jan),
            "Jan 1 should be inside winter: {jan}"
        );
    }

    #[test]
    fn the_year_runs_counter_clockwise() {
        let spring = deg(date(2026, 3, 20));
        let summer = deg(date(2026, 6, 20));
        assert!(
            spring > summer,
            "time must not advance clockwise: {spring} -> {summer}"
        );
    }

    #[test]
    fn cycle_covers_the_december_to_december_turn() {
        let (start, end) = cycle(date(2026, 1, 15));
        assert_eq!((start.year(), start.month(), start.day()), (2025, 12, 1));
        assert_eq!((end.year(), end.month(), end.day()), (2026, 12, 1));
    }
}
