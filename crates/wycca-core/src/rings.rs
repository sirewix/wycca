//! One ring, many layers. A ring is a span of time on an annulus — its scale
//! fixes the span, its layers divide it. Drawing a ring is then two steps and
//! nothing else: work out each layer's spans, and draw each span.
//!
//! Every angle runs through [`ring_angle`], so a layer can never disagree with
//! another about where something sits, and `reversed` is applied once, here,
//! rather than per layer.

use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, Months, NaiveTime, TimeZone, Timelike, Utc};

use crate::{
    config::{Anchor, BorderKind, Disp, Label, Layer, Location, Palette, Ring, Solar, Unit, Where},
    events,
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
    pub feeds: &'a BTreeMap<String, Vec<events::Event>>,
}

/// A ring's radial extent, resolved once by [`crate::layout`].
pub struct Band {
    pub r_in: f64,
    pub r_out: f64,
}

/// 1970-01-01, where the fixed-length units count from.
const EPOCH: DateTime<Utc> = DateTime::UNIX_EPOCH;

/// One stretch of a ring: where it starts, how long it is, and what to call it.
/// A zero length is an instant — a tick, a marker, or just somewhere to put a
/// label — and gets no fill.
struct Span {
    at: DateTime<Utc>,
    len: chrono::Duration,
    /// The distance to the next span of the same kind. An instant has no extent
    /// of its own, so this is what a label with `anchor: middle` takes the
    /// middle of: a month number stands in the middle of its month, not on its
    /// first day.
    cell: chrono::Duration,
    label: Option<String>,
}

impl Span {
    fn new(at: DateTime<Utc>, len: chrono::Duration, label: Option<&str>) -> Self {
        Self {
            at,
            len,
            cell: len,
            label: label.map(str::to_owned),
        }
    }

    /// What a label along this span measures: the span itself, or the gap to the
    /// next one when there is no span to measure.
    fn extent(&self) -> chrono::Duration {
        if self.len > chrono::Duration::zero() {
            self.len
        } else {
            self.cell
        }
    }
}

/// Where a span lands on a ring, worked out in one place so a fill, a stroke and
/// a label cannot disagree about it.
#[derive(Clone, Copy)]
struct Arc {
    /// The span's leading edge and its far edge: `at` plus its own length, in the
    /// scale's direction and flipped once by `reversed`.
    span: (f64, f64),
    /// The far edge of the whole cell, which is the same thing unless the span
    /// is an instant with no length of its own.
    cell: f64,
}

impl Arc {
    fn of(
        ring: &Ring,
        period: (DateTime<Utc>, DateTime<Utc>),
        at: DateTime<Utc>,
        span: &Span,
        per: f64,
    ) -> Self {
        let sweep = |len: chrono::Duration| {
            turn(
                ring.reversed,
                rad(ring.scale.dir() * len.num_milliseconds() as f64 / per * 360.0),
            )
        };
        let from = ring_angle(ring, period, at);
        Self {
            span: (from, from + sweep(span.len)),
            cell: from + sweep(span.extent()),
        }
    }
}

pub fn draw(svg: &mut Svg, ring: &Ring, band: &Band, ctx: &Ctx) {
    let period = ring.scale.period(ctx.now);
    let per = (period.1 - period.0).num_milliseconds().max(1) as f64;
    for layer in &ring.layers {
        let mut placed: Vec<f64> = Vec::new();
        for span in spans(layer, period, ctx) {
            // The arc is the span's start plus its length, in the scale's
            // direction and flipped once by `reversed`, so a span that runs past
            // the end of the period comes back round the turn.
            let arc = Arc::of(ring, period, span.at, &span, per);
            draw_span(svg, &span, arc, layer.disp(), band, ctx, &mut placed);
        }
    }
}

/// Every span a layer's source produces, in the order they happen.
fn spans(layer: &Layer, period: (DateTime<Utc>, DateTime<Utc>), ctx: &Ctx) -> Vec<Span> {
    match layer {
        Layer::Periodic {
            unit,
            amount,
            every,
            shift,
            ..
        } => periodic(*unit, *amount, *every, shift.index(*unit), period),
        Layer::Ics { source, only, .. } => ctx
            .feeds
            .get(source)
            .map(|feed| {
                feed.iter()
                    .filter(|e| only.is_none_or(|k| e.kind == k))
                    .filter(|e| e.date >= period.0.date_naive() && e.date <= period.1.date_naive())
                    .map(|e| {
                        let at = e.date.and_time(NaiveTime::MIN).and_utc();
                        Span::new(at, chrono::Duration::days(1), Some(&e.label))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Layer::Solar { what, .. } => solar(*what, period, ctx),
        Layer::Now { .. } => vec![Span::new(ctx.now, chrono::Duration::zero(), None)],
    }
}

/// Occurrence `i` of a pattern: `amount` units long, starting `shift` units into
/// the unit's cycle and repeating every `every` units.
///
/// The cycle is the calendar's, not the ring's, which is why a Saturday is a
/// Saturday on a month ring and a year ring alike: both count days from the same
/// Monday, so `shift: 5` is the same five days wherever it is read.
fn periodic(
    unit: Unit,
    amount: i64,
    every: i64,
    shift: i64,
    period: (DateTime<Utc>, DateTime<Utc>),
) -> Vec<Span> {
    let every = every.max(1);
    let (start, end) = period;
    // A span as long as the turn, or longer, cannot be drawn: see
    // `Layer::Periodic`. Months and years are counted by the calendar here, so
    // February is short and leap years are long without any special case.
    if amount != 0 && add(start, unit, amount) - start >= end - start {
        return Vec::new();
    }
    // The first occurrence that can overlap the period: it starts before the end
    // and reaches past the start, which is how a season or a night that began
    // last December is still on this year's ring. `guess` deliberately
    // overshoots, so the only walk needed is this one, back to the right one.
    // A span that ends exactly at the start touches nothing, so only an instant
    // can be as early as the start itself.
    let reaches = |i: i64| {
        let from = at(unit, every, shift, i);
        if amount == 0 {
            from >= start
        } else {
            add(from, unit, amount) > start
        }
    };
    let mut i = guess(unit, every, shift, start);
    while i > 0 && reaches(i - 1) {
        i -= 1;
    }
    let mut spans = Vec::new();
    while at(unit, every, shift, i) < end {
        let from = at(unit, every, shift, i);
        let mut span = Span::new(
            from,
            add(from, unit, amount) - from,
            Some(&label(unit, from)),
        );
        span.cell = add(from, unit, every) - from;
        spans.push(span);
        i += 1;
    }
    spans
}

/// An occurrence index at or just past the one covering `t`. Months and years
/// are counted by the calendar, so anything worked out from days alone is near
/// the answer at best — and the short divisors below are there to overshoot
/// rather than to be right, because `periodic` can only walk back.
fn guess(unit: Unit, every: i64, shift: i64, t: DateTime<Utc>) -> i64 {
    let days = (t - epoch(unit)).num_days();
    let units = match unit {
        Unit::Minute => days * 1440,
        Unit::Hour => days * 24,
        Unit::Day => days,
        Unit::Week => days / 7,
        Unit::Month => days / 28,
        Unit::Year => days / 364,
    };
    (units - shift).div_euclid(every) + 1
}

/// The instant of occurrence `i`, counted from the unit's epoch.
fn at(unit: Unit, every: i64, shift: i64, i: i64) -> DateTime<Utc> {
    add(epoch(unit), unit, shift + i * every)
}

/// What a periodic span is called: the hour, the day of the month, the month.
fn label(unit: Unit, at: DateTime<Utc>) -> String {
    (match unit {
        Unit::Minute => at.minute() as i64,
        Unit::Hour => at.hour() as i64,
        Unit::Day => at.day() as i64,
        Unit::Week => at.iso_week().week() as i64,
        Unit::Month => at.month() as i64,
        Unit::Year => at.year() as i64,
    })
    .to_string()
}

fn epoch(unit: Unit) -> DateTime<Utc> {
    match unit {
        Unit::Minute | Unit::Hour | Unit::Month | Unit::Year => EPOCH,
        // Days and weeks count from a Monday, so that index zero is one: that is
        // what makes `shift` read as the weekday on every ring.
        Unit::Day | Unit::Week => EPOCH + chrono::Duration::days(4),
    }
}

/// `n` units after `t`. Months and years go through the calendar, so a month is
/// as long as the month it starts in and February is short.
fn add(t: DateTime<Utc>, unit: Unit, n: i64) -> DateTime<Utc> {
    match unit {
        Unit::Minute => t + chrono::Duration::minutes(n),
        Unit::Hour => t + chrono::Duration::hours(n),
        Unit::Day => t + chrono::Duration::days(n),
        Unit::Week => t + chrono::Duration::weeks(n),
        Unit::Month => months(t, n),
        Unit::Year => months(t, n * 12),
    }
}

/// `n` calendar months after `t`, either way. `chrono::Months` is unsigned, so
/// the backwards half is a subtraction from the month before.
fn months(t: DateTime<Utc>, n: i64) -> DateTime<Utc> {
    let back = u32::try_from(-n).unwrap_or(u32::MAX);
    let fwd = u32::try_from(n).unwrap_or(0);
    let t = t.checked_sub_months(Months::new(back)).unwrap_or(t);
    t.checked_add_months(Months::new(fwd)).unwrap_or(t)
}

fn solar(what: Solar, period: (DateTime<Utc>, DateTime<Utc>), ctx: &Ctx) -> Vec<Span> {
    let name = what.name();
    match what {
        // Global instants, slid onto the wall clock by the UTC offset.
        Solar::Equinox | Solar::Solstice => sun::events_between(period.0, period.1)
            .into_iter()
            .filter(|(_, n)| *n == name)
            .map(|(t, _)| Span::new(ctx.wall(t), chrono::Duration::zero(), Some(name)))
            .collect(),
        // The Sun's own day: either it rises today or it does not.
        Solar::Sunrise | Solar::Sunset | Solar::Day | Solar::Night => {
            let Some((rise, set)) = daylight(ctx) else {
                return Vec::new();
            };
            let lit = set - rise;
            vec![match what {
                Solar::Sunrise => Span::new(ctx.wall(rise), chrono::Duration::zero(), Some(name)),
                Solar::Sunset => Span::new(ctx.wall(set), chrono::Duration::zero(), Some(name)),
                Solar::Day => Span::new(ctx.wall(rise), lit, Some(name)),
                _ => Span::new(ctx.wall(set), chrono::Duration::days(1) - lit, Some(name)),
            }]
        }
    }
}

fn daylight(ctx: &Ctx) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let loc = ctx.location?;
    sun::daylight(ctx.now.date_naive(), loc.lat, loc.lon)
}

/// The angle of `t` on a ring: its fraction of the period, in the scale's
/// direction, rotated by `offset_deg`, flipped once by `reversed`. Never clamped
/// and never folded into 0..360, because a span is allowed to run past the end
/// of the period and come back round — that is how a season or a night can
/// straddle the turn.
fn ring_angle(ring: &Ring, period: (DateTime<Utc>, DateTime<Utc>), t: DateTime<Utc>) -> f64 {
    let span = (period.1 - period.0).num_milliseconds().max(1) as f64;
    let frac = (t - period.0).num_milliseconds() as f64 / span;
    turn(
        ring.reversed,
        rad(ring.offset_deg + ring.scale.dir() * frac * 360.0),
    )
}
fn draw_span(
    svg: &mut Svg,
    span: &Span,
    arc: Arc,
    disp: &Disp,
    band: &Band,
    ctx: &Ctx,
    placed: &mut Vec<f64>,
) {
    let (cx, cy) = (ctx.cx, ctx.cy);
    let (r_in, r_out) = disp.band(band.r_in, band.r_out);
    let (a0, a1) = arc.span;

    if let Some(color) = &disp.background
        && span.len > chrono::Duration::zero()
    {
        // Trimmed at both ends so neighbours read as separate sectors, and only
        // while there is something left to draw. The length, not the sign: the
        // year ring runs the other way round.
        let gap = rad(disp.gap / 2.0);
        if (a1 - a0).abs() > 2.0 * gap {
            svg.path(
                &sector(cx, cy, r_in, r_out, a0 + gap, a1 - gap),
                ctx.palette.color(color),
            );
        }
    }

    if let Some(border) = &disp.border {
        let (from, to) = match border.kind {
            BorderKind::Line => (r_in, r_out),
            BorderKind::Hand => (border.inner * ctx.half, r_out),
        };
        svg.line(
            &radial(cx, cy, from, to, a0),
            ctx.palette.color(&border.color),
            border.width * ctx.half,
        );
    }

    if let Some(label) = &disp.label
        && let Some(text) = &span.label
    {
        let size = ctx.font_size * label.size;
        let r = label_radius(label, r_in, r_out, size);
        // Where along the span the words stand: on its leading edge, in the
        // middle of the arc, or at its far edge. The middle of the sweep, not of
        // the two angles folded into 0..360, which is not a thing.
        let a = match label.anchor {
            Anchor::Start => a0,
            Anchor::Middle => (a0 + arc.cell) / 2.0,
            Anchor::End => arc.cell,
        };
        let (x, y) = polar(cx, cy, r, a);
        let deg = a.to_degrees().rem_euclid(360.0);
        // Greedy: a label that would land on the last one is dropped, which is
        // all a ring of 365 day numbers can do.
        let wide =
            text.chars().count() as f64 * size * 0.55 / r.max(1.0) * 180.0 / std::f64::consts::PI;
        if placed.iter().all(|p| (deg - p).abs() > wide + 1.0) {
            placed.push(deg);
            svg.text(x, y, size, ctx.palette.color(&label.color), "middle", text);
        }
    }
}

/// Where a label sits: clear of the fill by its own height, so a bigger label
/// stands further off the ring rather than touching it.
fn label_radius(label: &Label, r_in: f64, r_out: f64, size: f64) -> f64 {
    let gap = size * 1.2;
    match label.at {
        Where::In => r_in - gap,
        Where::At => (r_in + r_out) / 2.0,
        Where::Out => r_out + gap,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Scale, Unit};
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
        ring_angle(&ring, scale.period(t), t).to_degrees()
    }

    /// A ring with no place to draw: every value zero, a clock to read, and a
    /// place for the Sun.
    fn ctx<'a>(
        now: DateTime<Utc>,
        palette: &'a Palette,
        feeds: &'a BTreeMap<String, Vec<events::Event>>,
    ) -> Ctx<'a> {
        Ctx {
            palette,
            now,
            t_ms: 0,
            cx: 0.0,
            cy: 0.0,
            half: 1.0,
            font_size: 1.0,
            utc_offset_ms: 0,
            location: None,
            feeds,
        }
    }

    #[test]
    fn the_year_starts_at_january_and_runs_backwards() {
        let jan = ring_angle_deg(Scale::Year, date(2027, 1, 1), false);
        let feb15 = ring_angle_deg(Scale::Year, date(2027, 2, 15), false);
        assert!(jan.abs() < 1e-6, "January starts at the top: {jan}");
        assert!(feb15 < jan, "the year must not run clockwise: {feb15}");
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

    #[test]
    fn a_period_covers_its_own_turn_and_nothing_more() {
        let year = Scale::Year.period(date(2026, 6, 15));
        // Twelve months on a year ring, seven days on a week ring, and the
        // hours of one day.
        let got =
            |unit, amount, every, shift, period| periodic(unit, amount, every, shift, period).len();
        assert_eq!(got(Unit::Month, 1, 1, 0, year), 12);
        assert_eq!(got(Unit::Month, 0, 3, 0, year), 4);
        assert_eq!(got(Unit::Day, 0, 1, 0, year), 365);
        assert_eq!(
            got(Unit::Hour, 0, 1, 0, Scale::Hour24.period(date(2026, 6, 15))),
            24
        );
        assert_eq!(
            got(Unit::Day, 1, 1, 0, Scale::Week.period(date(2026, 6, 15))),
            7
        );
    }

    #[test]
    fn a_weekend_is_two_days_from_saturday_on_any_ring() {
        // The same Saturdays on every ring, because days count from a Monday
        // everywhere and not from the start of whatever turn is being drawn: a
        // month ring gets its four, a year ring its fifty-two, a week ring one.
        for scale in [Scale::Week, Scale::Month, Scale::Year] {
            let spans = periodic(Unit::Day, 2, 7, 5, scale.period(date(2026, 6, 17)));
            assert!(!spans.is_empty(), "{scale:?}");
            assert!(
                spans
                    .iter()
                    .all(|s| s.at.weekday().num_days_from_monday() == 5),
                "{scale:?}: weekends start on a Saturday"
            );
            assert!(
                spans.iter().all(|s| s.len == chrono::Duration::days(2)),
                "{scale:?}: Saturday runs into Sunday"
            );
            // And the next one is a week later, not tomorrow.
            assert!(
                spans
                    .windows(2)
                    .all(|w| w[1].at - w[0].at == chrono::Duration::days(7)),
                "{scale:?}: weekends repeat weekly"
            );
        }
        let week = periodic(Unit::Day, 2, 7, 5, Scale::Week.period(date(2026, 6, 17)));
        assert_eq!(week.len(), 1);
    }

    #[test]
    fn a_span_longer_than_the_turn_is_not_drawn() {
        // A season is three months, which is most of a month ring.
        assert!(
            periodic(
                Unit::Month,
                3,
                12,
                11,
                Scale::Month.period(date(2026, 6, 15))
            )
            .is_empty()
        );
        // On a year ring both Decembers reach into the turn, and they are the
        // same arc twice: one starts before the period, one starts inside it.
        assert_eq!(
            periodic(
                Unit::Month,
                3,
                12,
                11,
                Scale::Year.period(date(2026, 6, 15))
            )
            .len(),
            2
        );
    }

    /// Winter starts in December, crosses the turn, and is still one arc: the
    /// season sector and the month border come from the same instants, so they
    /// cannot drift apart.
    #[test]
    fn winter_is_one_sector_straddling_the_top_of_the_year() {
        let period = Scale::Year.period(date(2026, 6, 15));
        let spans = periodic(Unit::Month, 3, 12, 11, period);
        // Both Decembers reach into the turn, and they are the same arc twice:
        // the ring repeats, so December 2025 and December 2026 land identically.
        assert_eq!(spans.len(), 2);
        let winter = &spans[1];
        assert_eq!((winter.at.year(), winter.at.month()), (2026, 12));
        // Three months by the calendar, leap year or not.
        assert_eq!((winter.at + winter.len).month(), 3);

        let ring = Ring::new(Scale::Year);
        // Both edges, folded back into one turn so the crossing does not read as
        // a sign change.
        let deg = |t| ring_angle(&ring, period, t).to_degrees().rem_euclid(360.0);
        let (from, to) = (deg(winter.at), deg(winter.at + winter.len));
        // The year runs counter-clockwise, so December is just past the top and
        // March is that far the other side of it: winter straddles twelve.
        assert!(
            from < 45.0 && to > 300.0,
            "winter is not round the top: {from}..{to}"
        );
        let mid = ((from + (to - 360.0)) / 2.0).rem_euclid(360.0);
        assert!(
            mid.min(360.0 - mid) < 20.0,
            "winter should sit on the top, not at {mid}"
        );
    }

    #[test]
    fn an_instant_label_is_the_number_the_pattern_gave_it() {
        let period = Scale::Hour24.period(date(2026, 6, 15));
        let spans = periodic(Unit::Hour, 0, 6, 0, period);
        assert_eq!(
            spans
                .iter()
                .filter_map(|s| s.label.clone())
                .collect::<Vec<_>>(),
            ["0", "6", "12", "18"]
        );
        // And the days of a month ring, which is what a month ring's numbers are.
        let month = periodic(Unit::Day, 0, 1, 0, Scale::Month.period(date(2026, 2, 15)));
        assert_eq!(month.len(), 28, "February 2026");
        assert_eq!(month[0].label.as_deref(), Some("1"));
        assert_eq!(month[27].label.as_deref(), Some("28"));
    }

    #[test]
    fn daylight_and_night_are_two_spans_that_make_the_day() {
        let palette = Palette::resolve(crate::Theme::Dark, &Default::default());
        let feeds = BTreeMap::new();
        let now = date(2026, 6, 15) + chrono::Duration::hours(12);
        let mut ctx = ctx(now, &palette, &feeds);
        ctx.location = Some(crate::Location {
            lat: 52.52,
            lon: 13.405,
        });
        let period = Scale::Hour24.period(now);
        let (day, night) = (
            solar(Solar::Day, period, &ctx),
            solar(Solar::Night, period, &ctx),
        );
        assert_eq!(day.len(), 1);
        assert_eq!(day[0].len + night[0].len, chrono::Duration::days(1));
        assert!(
            day[0].len > chrono::Duration::hours(16),
            "midsummer is more lit than not: {:?}",
            day[0].len
        );
        // Without a place there is no sunrise, and no span to draw.
        ctx.location = None;
        assert!(solar(Solar::Day, period, &ctx).is_empty());
    }

    #[test]
    fn a_layer_with_nothing_to_draw_draws_nothing() {
        let bare: crate::Config =
            serde_json::from_str(r#"{"rings":[{"scale":"week","layers":[{"kind":"now"}]}]}"#)
                .unwrap();
        let none: crate::Config = serde_json::from_str(r#"{"rings":[{"scale":"week"}]}"#).unwrap();
        assert_eq!(
            crate::render(&bare, 0),
            crate::render(&none, 0),
            "one empty layer is no layer"
        );
    }

    /// Every season fills the year ring, and the year runs the other way round,
    /// so a fill whose arc is measured backwards is still a fill. (It was not,
    /// once: the gap guard compared the two ends of an arc without their order.)
    #[test]
    fn a_label_can_stand_in_the_middle_of_its_span() {
        // Seven day numbers round a week ring, told apart by where they sit: on
        // the boundary, in the middle of the day, at its far edge.
        let x_of_first = |anchor: &str| {
            let cfg: crate::Config = serde_json::from_str(&format!(
                r#"{{"rings":[{{"scale":"week","layers":[{{"kind":"periodic","unit":"day",
                     "shift":"mon","disp":{{"label":{{"anchor":"{anchor}"}}}}}}]}}]}}"#
            ))
            .unwrap();
            let svg = crate::render(&cfg, 1_800_000_000_000);
            let at = svg.find("<text x=\"").expect("a label") + 9;
            // Measured from the middle of the canvas, which is where the top of
            // a ring is: x = half the size when a label sits at twelve o'clock.
            let x: f64 = svg[at..].split('"').next().unwrap().parse().unwrap();
            x - 512.0
        };
        // Monday starts at the top of the ring, so a label on its start is
        // straight up the middle and the other two are a twelfth of a week away.
        let start = x_of_first("start");
        let middle = x_of_first("middle");
        let end = x_of_first("end");
        assert!(start.abs() < 1.0, "on the boundary: {start}");
        assert!(middle > 20.0, "in the middle of the day: {middle}");
        assert!(end > middle * 1.8, "at the far edge: {end} vs {middle}");
    }

    #[test]
    fn an_instant_label_takes_the_middle_of_the_gap_to_the_next_one() {
        // A month number is a zero-length span on the first of a month, which has
        // no middle of its own — so `middle` means the middle of the cell, which
        // is what "in the middle of the month" is.
        let x_of_first = |anchor: &str| {
            let cfg: crate::Config = serde_json::from_str(&format!(
                r#"{{"rings":[{{"scale":"year","offset_deg":0,"layers":[{{"kind":"periodic",
                     "unit":"month","amount":0,"shift":"jan","disp":{{"label":{{"anchor":"{anchor}"}}}}}}]}}]}}"#
            ))
            .unwrap();
            let svg = crate::render(&cfg, 1_800_000_000_000);
            let at = svg.find("<text x=\"").expect("a label") + 9;
            svg[at..].split('"').next().unwrap().parse::<f64>().unwrap() - 512.0
        };
        // The year runs counter-clockwise, so the middle of January is left of
        // the top and the end of January further left still.
        let start = x_of_first("start");
        let middle = x_of_first("middle");
        let end = x_of_first("end");
        assert!(start.abs() < 1.0, "on the first: {start}");
        assert!(middle < -20.0, "half way through: {middle}");
        assert!(end < middle * 1.8, "on the last: {end} vs {middle}");
    }

    #[test]
    fn the_seasons_fill_the_year_ring() {
        let palette = Palette::resolve(crate::Theme::Dark, &Default::default());
        let cfg = crate::Config {
            rings: vec![Ring::new(Scale::Year)],
            ..Default::default()
        };
        let svg = crate::render(&cfg, 1_800_000_000_000);
        for slot in ["winter", "spring", "summer", "autumn"] {
            assert!(
                svg.contains(&format!("fill=\"{}\"", palette.color(slot))),
                "{slot} is missing from the year ring"
            );
        }
    }
}
