//! CF time units (`"<unit> since <reference>"`) and their conversion to the
//! output's time base, days since 1950-01-01 00:00:00 UTC.

use std::error::Error;

use chrono::{FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};

/// Linear map from raw time values to days since 1950-01-01:
/// `days = raw * days_per_unit + offset_days`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeScale {
    pub days_per_unit: f64,
    pub offset_days: f64,
}

impl TimeScale {
    /// The Copernicus time base: values are already days since 1950-01-01.
    pub const DAYS_SINCE_1950: TimeScale = TimeScale { days_per_unit: 1.0, offset_days: 0.0 };

    pub fn is_identity(&self) -> bool {
        *self == Self::DAYS_SINCE_1950
    }

    pub fn apply(&self, raw: f64) -> f64 {
        raw * self.days_per_unit + self.offset_days
    }

    /// Parse CF units such as `"seconds since 1970-01-01 00:00:00"`,
    /// `"days since 1950-01-01T00:00:00Z"` or `"hours since 2000-1-1 06:00 +01:00"`.
    /// Supported units: days, hours, minutes, seconds, milliseconds (and their
    /// usual abbreviations). Months and years are rejected because CF does not
    /// give them a fixed length.
    pub fn parse(units: &str) -> Result<TimeScale, Box<dyn Error>> {
        let bad = || format!("unsupported time units \"{units}\": expected \"<unit> since <date>\", e.g. \"seconds since 1970-01-01 00:00:00\"");
        let lower = units.trim().to_ascii_lowercase();
        let (unit, reference) = lower.split_once(" since ").ok_or_else(bad)?;

        let days_per_unit = match unit.trim() {
            "days" | "day" | "d" => 1.0,
            "hours" | "hour" | "hrs" | "hr" | "h" => 1.0 / 24.0,
            "minutes" | "minute" | "mins" | "min" => 1.0 / 1_440.0,
            "seconds" | "second" | "secs" | "sec" | "s" => 1.0 / 86_400.0,
            "milliseconds" | "millisecond" | "msecs" | "msec" | "ms" => 1.0 / 86_400_000.0,
            "months" | "month" | "years" | "year" => {
                return Err(format!("time units \"{units}\": months and years have no fixed length; use days or seconds").into())
            }
            _ => return Err(bad().into()),
        };

        let reference_ms = parse_reference(reference.trim()).ok_or_else(bad)?;
        let epoch_ms = Utc.with_ymd_and_hms(1950, 1, 1, 0, 0, 0).unwrap().timestamp_millis();
        let offset_days = (reference_ms - epoch_ms) as f64 / 86_400_000.0;

        Ok(TimeScale { days_per_unit, offset_days })
    }
}

/// Parse a CF reference date-time to Unix milliseconds. Accepts `YYYY-M-D`,
/// optionally followed (after `T` or a space) by `H:M[:S[.fff]]`, and an
/// optional time zone: `Z`, `UTC`, `GMT`, or a `+HH[:MM]` / `-HHMM` offset.
fn parse_reference(s: &str) -> Option<i64> {
    let mut parts = s.split_whitespace();

    // `1950-01-01T00:00:00Z`: the date token may carry the time after a `t`.
    let first = parts.next()?;
    let (date_str, time_str) = match first.split_once('t') {
        Some((d, t)) => (d, Some(t)),
        None => (first, None),
    };
    let date = parse_date(date_str)?;
    let mut time = NaiveTime::MIN;
    let mut offset_secs = 0;

    for part in time_str.into_iter().chain(parts) {
        if let Some(off) = parse_offset(part) {
            offset_secs = off;
        } else if let Some(t) = parse_time(part) {
            time = t;
        } else if let Some(i) = part.find(['+', '-']).filter(|&i| i > 0) {
            // `00:00:00+01:00`: time and offset glued together.
            time = parse_time(&part[..i])?;
            offset_secs = parse_offset(&part[i..])?;
        } else {
            return None;
        }
    }

    let local = NaiveDateTime::new(date, time);
    let offset = FixedOffset::east_opt(offset_secs)?;
    Some(offset.from_local_datetime(&local).single()?.timestamp_millis())
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    let mut it = s.splitn(3, '-');
    let y = it.next()?.parse().ok()?;
    let m = it.next()?.parse().ok()?;
    let d = it.next()?.parse().ok()?;
    NaiveDate::from_ymd_opt(y, m, d)
}

/// `H:M`, `H:M:S` or `H:M:S.fff`; a trailing `z` (from `...T00:00:00Z`) is allowed.
fn parse_time(s: &str) -> Option<NaiveTime> {
    let s = s.strip_suffix('z').unwrap_or(s);
    let mut it = s.split(':');
    let h = it.next()?.parse().ok()?;
    let m = it.next()?.parse().ok()?;
    let sec: f64 = match it.next() {
        Some(x) => x.parse().ok()?,
        None => 0.0,
    };
    if it.next().is_some() || !(0.0..60.0).contains(&sec) {
        return None;
    }
    let whole = sec.trunc() as u32;
    let nanos = ((sec - sec.trunc()) * 1e9).round() as u32;
    NaiveTime::from_hms_nano_opt(h, m, whole, nanos)
}

/// Time zone token to an offset east of UTC in seconds.
fn parse_offset(s: &str) -> Option<i32> {
    match s {
        "z" | "utc" | "gmt" => return Some(0),
        _ => {}
    }
    let (sign, rest) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => return None,
    };
    let (h, m): (i32, i32) = if let Some((h, m)) = rest.split_once(':') {
        (h.parse().ok()?, m.parse().ok()?)
    } else if rest.len() == 4 {
        (rest[..2].parse().ok()?, rest[2..].parse().ok()?)
    } else {
        (rest.parse().ok()?, 0)
    };
    Some(sign * (h * 3600 + m * 60))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn days(units: &str, raw: f64) -> f64 {
        TimeScale::parse(units).unwrap().apply(raw)
    }

    #[test]
    fn copernicus_units_are_identity() {
        assert!(TimeScale::parse("days since 1950-01-01T00:00:00Z").unwrap().is_identity());
        assert!(TimeScale::parse("days since 1950-01-01 00:00:00 UTC").unwrap().is_identity());
        assert!(TimeScale::parse("days since 1950-01-01").unwrap().is_identity());
    }

    #[test]
    fn unix_seconds() {
        // 1970-01-01 is 7305 days after 1950-01-01.
        assert_eq!(days("seconds since 1970-01-01 00:00:00", 0.0), 7305.0);
        assert_eq!(days("seconds since 1970-01-01", 86_400.0), 7306.0);
        assert_eq!(days("s since 1970-1-1", 43_200.0), 7305.5);
    }

    #[test]
    fn hours_minutes_milliseconds() {
        assert_eq!(days("hours since 1950-01-02", 12.0), 1.5);
        assert_eq!(days("minutes since 1950-01-01 00:00", 1_440.0), 1.0);
        assert_eq!(days("milliseconds since 1950-01-01", 86_400_000.0), 1.0);
    }

    #[test]
    fn reference_time_and_offset() {
        assert_eq!(days("days since 1950-01-01 12:00:00", 0.0), 0.5);
        assert_eq!(days("days since 1950-01-01T12:00:00Z", 0.0), 0.5);
        // 06:00 at +06:00 is midnight UTC.
        assert_eq!(days("days since 1950-01-01 06:00 +06:00", 0.0), 0.0);
        assert_eq!(days("days since 1950-01-01 00:00 -0600", 0.0), 0.25);
        assert_eq!(days("days since 1950-01-01T06:00:00+06:00", 0.0), 0.0);
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(days("Seconds Since 1970-01-01T00:00:00Z", 0.0), 7305.0);
    }

    #[test]
    fn rejects_bad_units() {
        assert!(TimeScale::parse("days").is_err());
        assert!(TimeScale::parse("fortnights since 1950-01-01").is_err());
        assert!(TimeScale::parse("months since 1950-01-01").is_err());
        assert!(TimeScale::parse("days since yesterday").is_err());
        assert!(TimeScale::parse("days since 1950-13-01").is_err());
    }
}
