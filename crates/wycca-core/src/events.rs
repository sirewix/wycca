//! Full-day events from an iCalendar feed.
//!
//! Only `DTSTART;VALUE=DATE` counts: a ring has no room for a time of day, and the
//! feeds worth reading (Google's per-country holiday calendars) are all day-long.
//! `TRANSP:TRANSPARENT` is what separates time off from a busy day, which is the
//! distinction the year ring draws.
//!
//! ponytail: no `RRULE` expansion — recurring all-day events are read once, at
//! their first date. Country holiday feeds enumerate every year explicitly, so
//! nothing is lost there; swap the parser for the `icalendar` crate if someone
//! actually wants a birthday calendar.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// `TRANSP:TRANSPARENT`: a day off. This is what a holiday is.
    #[default]
    Free,
    /// A day that is not free but is still a whole day we want on the ring.
    Busy,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub date: NaiveDate,
    pub label: String,
    #[serde(default)]
    pub kind: Kind,
}

/// Every whole-day event in the feed, sorted by date, duplicates dropped.
pub fn parse(src: &str) -> Vec<Event> {
    let unfolded = unfold(src);
    let lines: Vec<&str> = unfolded.iter().map(String::as_str).collect();
    let mut events: Vec<Event> = lines
        .split(|l| *l == "BEGIN:VEVENT" || *l == "END:VEVENT")
        .filter(|block| !block.is_empty())
        .filter_map(|block| {
            let start = all_day(block, "DTSTART")?;
            let kind = match prop(block, "TRANSP") {
                Some("TRANSPARENT") => Kind::Free,
                _ => Kind::Busy,
            };
            let label = unescape(prop(block, "SUMMARY").unwrap_or("").trim());
            // DTEND is exclusive; absent means a single day.
            let end = all_day(block, "DTEND").unwrap_or_else(|| start.succ_opt().unwrap_or(start));
            Some((
                start,
                end.max(start.succ_opt().unwrap_or(start)),
                kind,
                label,
            ))
        })
        .flat_map(|(start, end, kind, label)| {
            let mut days = Vec::new();
            let mut day = start;
            while day < end {
                days.push(Event {
                    date: day,
                    label: label.clone(),
                    kind,
                });
                match day.succ_opt() {
                    Some(next) => day = next,
                    None => break,
                }
            }
            days
        })
        .collect();
    events.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.label.cmp(&b.label)));
    events.dedup_by(|a, b| a.date == b.date && a.label == b.label);
    events
}

/// Events falling in `[from, to]`, inclusive.
pub fn parse_window(src: &str, from: NaiveDate, to: NaiveDate) -> Vec<Event> {
    parse(src)
        .into_iter()
        .filter(|e| e.date >= from && e.date <= to)
        .collect()
}

/// RFC 5545 folds long lines and may use CRLF; undo both.
fn unfold(src: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in src.replace("\r\n", "\n").lines() {
        if let Some(rest) = line.strip_prefix([' ', '\t']) {
            if let Some(prev) = lines.last_mut() {
                prev.push_str(rest);
            }
        } else {
            lines.push(line.to_owned());
        }
    }
    lines
}

fn prop<'a>(block: &'a [&'a str], name: &str) -> Option<&'a str> {
    block.iter().find_map(|line| {
        let (head, value) = line.split_once(':')?;
        (head.split(';').next() == Some(name)).then_some(value)
    })
}

/// A `DTSTART`/`DTEND` that is a bare date rather than a timestamp.
fn all_day(block: &[&str], name: &str) -> Option<NaiveDate> {
    block.iter().find_map(|line| {
        let (head, value) = line.split_once(':')?;
        let mut params = head.split(';');
        if params.next()? != name || params.all(|p| p != "VALUE=DATE") {
            return None;
        }
        NaiveDate::parse_from_str(value.trim(), "%Y%m%d").ok()
    })
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push(' '),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trimmed to the shape Google actually serves: CRLF, folded SUMMARY,
    /// escaped commas, a timed event, and an exclusive DTEND.
    const FEED: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\n\
        BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20260101\r\nDTEND;VALUE=DATE:20260102\r\n\
        SUMMARY:Republic Day\r\nTRANSP:TRANSPARENT\r\nEND:VEVENT\r\n\
        BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20260102\r\nDTEND;VALUE=DATE:20260104\r\n\
        SUMMARY:New Year, part two\r\nTRANSP:TRANSPARENT\r\nEND:VEVENT\r\n\
        BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20260509\r\nDTEND;VALUE=DATE:20260510\r\n\
        SUMMARY:Victory Day\r\nEND:VEVENT\r\n\
        BEGIN:VEVENT\r\nDTSTART:20260601T090000Z\r\nDTEND:20260601T100000Z\r\n\
        SUMMARY:Meeting\r\nEND:VEVENT\r\n\
        BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20260706\r\n\
        SUMMARY:Capital City Day\r\nTRANSP:TRANSPARENT\r\nEND:VEVENT\r\n\
        END:VCALENDAR\r\n";

    #[test]
    fn whole_days_including_multi_day_ones() {
        let events = parse(FEED);
        assert_eq!(
            events.iter().map(|e| e.date).collect::<Vec<_>>(),
            vec![
                NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 1, 2).unwrap(),
                NaiveDate::from_ymd_opt(2026, 1, 3).unwrap(),
                NaiveDate::from_ymd_opt(2026, 5, 9).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 6).unwrap(),
            ]
        );
    }

    #[test]
    fn transparent_is_time_off_and_everything_else_is_not() {
        let events = parse(FEED);
        assert_eq!(events[0].kind, Kind::Free);
        assert_eq!(events[3].kind, Kind::Busy, "Victory Day has no TRANSP");
    }

    #[test]
    fn timed_events_are_skipped() {
        assert!(!parse(FEED).iter().any(|e| e.label == "Meeting"));
    }

    #[test]
    fn a_missing_dtend_is_a_single_day() {
        let events = parse(FEED);
        assert_eq!(
            events
                .iter()
                .filter(|e| e.label == "Capital City Day")
                .count(),
            1
        );
    }

    #[test]
    fn the_window_keeps_the_rest_out() {
        let from = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        assert_eq!(parse_window(FEED, from, to).len(), 3);
    }

    #[test]
    fn folded_lines_and_escapes_are_reassembled() {
        let folded = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20260321\r\n\
            SUMMARY:A rather long holiday name that Google\r\n  wraps across lines\r\n\
            END:VEVENT\r\nEND:VCALENDAR\r\n";
        assert_eq!(
            parse(folded)[0].label,
            "A rather long holiday name that Google wraps across lines"
        );
        assert_eq!(unescape(r"New Year\, part two"), "New Year, part two");
        assert_eq!(unescape(r"two\nlines"), "two lines");
    }
}
