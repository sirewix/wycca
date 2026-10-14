//! Equinoxes and solstices, from the apparent solar longitude (Meeus ch. 25,
//! low precision — good to about 15 minutes).
//!
//! No latitude, no timezone: these are global instants, the Sun crosses 0/90/180/270
//! degrees of ecliptic longitude once a year wherever you are. Only *where they land
//! on a wall clock* needs the offset, which is what `Config::utc_offset_ms` is for.

use std::f64::consts::PI;

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, Utc};

/// The four events, in the order they happen: March equinox, June solstice,
/// September equinox, December solstice.
pub const EVENTS: [(f64, &str); 4] = [
    (0.0, "equinox"),
    (90.0, "solstice"),
    (180.0, "equinox"),
    (270.0, "solstice"),
];

/// Apparent solar longitude in degrees, unwrapped so it climbs monotonically for
/// centuries. 0 is the March equinox, but by 2026 the raw number is ~9640.
fn solar_longitude(t: DateTime<Utc>) -> f64 {
    // Julian centuries since J2000. The correction terms are per century, not per day.
    let c = (t.timestamp() as f64 / 86_400.0 + 2_440_587.5 - 2_451_545.0) / 36_525.0;
    let mean_long = 280.46646 + 36_000.769_83 * c;
    let anomaly = (357.52911 + 35_999.050_29 * c).to_radians();
    let centre = (1.914602 - 0.004817 * c) * anomaly.sin()
        + (0.019_993 - 0.000_101 * c) * (2.0 * anomaly).sin()
        + 0.000_289 * (3.0 * anomaly).sin();
    let omega = (125.04 - 1934.136 * c).to_radians();
    mean_long + centre - 0.00569 - 0.004_78 * omega.sin()
}

fn utc(y: i32, m: u32, d: u32) -> DateTime<Utc> {
    NaiveDate::from_ymd_opt(y, m, d)
        .unwrap_or_default()
        .and_time(NaiveTime::MIN)
        .and_utc()
}

/// The instant during `year` when the apparent solar longitude reaches `target`.
pub fn event(year: i32, target: f64) -> DateTime<Utc> {
    let start = utc(year, 1, 1);
    // Target measured from the start of the year, wrapped up to the first whole
    // turn that is still ahead of it. The slowest is the December solstice, 355
    // days in, hence the window.
    let goal = target + 360.0 * ((solar_longitude(start) - target) / 360.0).ceil();
    let (mut lo, mut hi) = (start, start + Duration::days(400));
    for _ in 0..40 {
        let mid = lo + (hi - lo) / 2;
        if solar_longitude(mid) < goal {
            lo = mid
        } else {
            hi = mid
        };
    }
    lo
}

/// Every event between `from` and `to` inclusive, each with its name.
pub fn events_between(
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Vec<(DateTime<Utc>, &'static str)> {
    (from.year()..=to.year())
        .flat_map(|year| EVENTS.map(|(deg, name)| (event(year, deg), name)))
        .filter(|(t, _)| *t >= from && *t <= to)
        .collect()
}

/// Sunrise and sunset on `date`, as UTC instants, or `None` when the Sun does not
/// cross the horizon at all (polar day or polar night).
///
/// NOAA's sunrise equation, good to about a minute. Unlike the solstices above
/// this one genuinely needs a place: `lat` degrees north, `lon` degrees east.
pub fn daylight(date: NaiveDate, lat: f64, lon: f64) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    // Fractional year, 2pi per day.
    let g = 2.0 * PI / 365.0 * (date.ordinal() as f64 - 1.0);
    let (s1, c1) = (g.sin(), g.cos());
    let (s2, c2) = ((2.0 * g).sin(), (2.0 * g).cos());
    let (s3, c3) = ((3.0 * g).sin(), (3.0 * g).cos());
    // Equation of time, in minutes.
    let eqtime =
        229.18 * (0.000_075 + 0.001_868 * c1 - 0.032_077 * s1 - 0.014_615 * c2 - 0.040_849 * s2);
    // Solar declination, in radians.
    let decl = 0.006_918 - 0.399_912 * c1 + 0.070_257 * s1 - 0.006_758 * c2 + 0.000_907 * s2
        - 0.002_697 * c3
        + 0.001_48 * s3;

    let lat = lat.to_radians();
    // 90.833 degrees: the Sun's radius, plus refraction and the usual margin.
    let cos_h = (90.833_f64).to_radians().cos() / (lat.cos() * decl.cos()) - lat.tan() * decl.tan();
    // Outside the range means the Sun never rose or never set.
    if cos_h.abs() >= 1.0 {
        return None;
    }
    let ha = cos_h.acos().to_degrees();
    // Minutes from UTC midnight, either side of solar noon.
    let minutes = |sign: f64| 720.0 - 4.0 * (lon + sign * ha) - eqtime;
    let at = |mins: f64| {
        date.and_time(NaiveTime::MIN).and_utc() + Duration::seconds((mins * 60.0).round() as i64)
    };
    Some((at(minutes(1.0)), at(minutes(-1.0))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn hours(date: NaiveDate, lat: f64, lon: f64) -> f64 {
        let (rise, set) = daylight(date, lat, lon).expect("the sun rises here");
        (set - rise).num_seconds() as f64 / 3600.0
    }

    #[test]
    fn the_equinox_is_always_about_twelve_hours() {
        // Anywhere on Earth, give or take refraction and the equation of time.
        for (lat, lon) in [(0.0, 0.0), (52.52, 13.405), (-33.87, 151.21), (64.1, -21.9)] {
            for date in [date(2026, 3, 20), date(2026, 9, 22)] {
                let h = hours(date, lat, lon);
                assert!((h - 12.0).abs() < 0.5, "{lat}/{lon} {date}: {h} h");
            }
        }
    }

    #[test]
    fn june_is_the_longest_and_december_the_shortest() {
        let (lat, lon) = (52.52, 13.405);
        let june = hours(date(2026, 6, 21), lat, lon);
        let march = hours(date(2026, 3, 21), lat, lon);
        let december = hours(date(2026, 12, 21), lat, lon);
        assert!(
            june > march && march > december,
            "{june} {march} {december}"
        );
        assert!(
            (june - 16.9).abs() < 0.2,
            "Berlin midsummer is ~16h52m, got {june}"
        );
        assert!(
            (december - 7.7).abs() < 0.2,
            "Berlin midwinter is ~7h41m, got {december}"
        );
    }

    #[test]
    fn the_equator_has_no_seasons() {
        let longest = hours(date(2026, 6, 21), 0.0, 0.0);
        let shortest = hours(date(2026, 12, 21), 0.0, 0.0);
        assert!((longest - shortest).abs() < 0.3, "{longest} vs {shortest}");
    }

    #[test]
    fn the_poles_have_days_the_ring_cannot_draw() {
        // Polar night *and* midnight sun both mean there is no sunrise or sunset.
        assert!(
            daylight(date(2026, 12, 21), 78.0, 15.0).is_none(),
            "polar night"
        );
        assert!(
            daylight(date(2026, 6, 21), 78.0, 15.0).is_none(),
            "midnight sun"
        );
        assert!(
            daylight(date(2026, 6, 21), 60.0, 15.0).is_some(),
            "an ordinary high-latitude summer"
        );
    }

    #[test]
    fn sunrise_and_sunset_bracket_solar_noon() {
        let (rise, set) = daylight(date(2026, 6, 21), 52.52, 13.405).unwrap();
        // Solar noon: 12:00 less the longitude, less the equation of time.
        let noon = date(2026, 6, 21).and_time(NaiveTime::MIN).and_utc()
            + Duration::milliseconds(((720.0 - 13.405 * 4.0 + 1.3) * 60_000.0) as i64);
        assert!(rise < noon && noon < set, "{rise} {noon} {set}");
        // Symmetric about it, to well inside the formula's own accuracy.
        let skew = (noon - rise) - (set - noon);
        assert!(skew.num_seconds().abs() <= 300, "skewed by {skew}");
    }

    fn within_days(
        year: i32,
        target_deg: f64,
        month: u32,
        day: u32,
        tolerance: i64,
    ) -> DateTime<Utc> {
        let t = event(year, target_deg);
        let expected = utc(year, month, day);
        assert!(
            (t - expected).num_days().abs() <= tolerance,
            "{target_deg} deg landed on {} instead of {month}-{day}",
            t.format("%Y-%m-%d")
        );
        t
    }

    #[test]
    fn events_land_on_the_dates_the_almanac_prints() {
        // Northern hemisphere, 2026. One day of slack: the exact instant moves
        // between sources, and this series is good to a few minutes.
        within_days(2026, 0.0, 3, 20, 1);
        within_days(2026, 90.0, 6, 21, 1);
        within_days(2026, 180.0, 9, 22, 1);
        within_days(2026, 270.0, 12, 21, 1);
    }

    #[test]
    fn the_longest_day_is_june_not_july() {
        let june = event(2026, 90.0);
        let july = utc(2026, 7, 1);
        assert!(june < july);
        assert!(june.day() >= 20 && june.day() <= 22);
    }

    #[test]
    fn the_window_keeps_december_out_of_the_next_cycle() {
        let cycle = utc(2025, 12, 1);
        let found = events_between(cycle, utc(2026, 12, 1));
        assert_eq!(found.len(), 4);
        assert_eq!(
            found.last().map(|(t, name)| (t.year(), t.month(), *name)),
            Some((2026, 9, "equinox"))
        );
    }
}
