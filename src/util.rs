/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use std::io::{self, Write};

/// Default daily target (8 hours) in seconds. Used when no `--target` is given
/// and for sessions stored before per-session targets existed.
pub const DEFAULT_TARGET_SECS: i64 = 8 * 3600;

/// Tool name for printing.
pub const TOOL_NAME: &str = "zeit";

/// A reporting period: a whole year, a month, or a single day.
#[derive(Debug, Clone, Copy)]
pub enum Period {
    Year(i32),
    Month(i32, u32),
    Week(i32, u32),
    Day(NaiveDate),
}

impl Period {
    /// Human-readable form: `2026`, `2026-05` or `2026-05-27`.
    pub fn label(&self) -> String {
        match self {
            Period::Year(y) => format!("{y}"),
            Period::Month(y, m) => format!("{y}-{m:02}"),
            Period::Week(y, w) => format!("{y}-W{w:02}"),
            Period::Day(d) => d.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum At {
    Time(NaiveTime),
    DateTime(NaiveDateTime),
}

impl At {
    pub fn resolve(self, default_date: NaiveDate) -> Result<DateTime<Local>> {
        match self {
            Self::Time(time) => local_datetime(default_date, time),
            Self::DateTime(datetime) => local_datetime(datetime.date(), datetime.time()),
        }
    }
}

/// Parse "HH:MM" or "HH:MM:SS" into a NaiveTime.
pub fn parse_time(s: &str) -> Result<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
        .with_context(|| format!("invalid time {s:?} - expected HH:MM or HH:MM:SS"))
}

/// Parse a time or an explicit local date and time.
pub fn parse_at(s: &str) -> Result<At> {
    if let Ok(time) = parse_time(s) {
        return Ok(At::Time(time));
    }
    for format in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(datetime) = NaiveDateTime::parse_from_str(s, format) {
            return Ok(At::DateTime(datetime));
        }
    }
    bail!("invalid time {s:?} - expected HH:MM, HH:MM:SS or YYYY-MM-DDTHH:MM[:SS]")
}

/// Parse a daily target given as "HH:MM" (e.g. "4:00", "04:30", "8:00") into seconds.
pub fn parse_target(s: &str) -> Result<i64> {
    let (h, m) = s
        .split_once(':')
        .ok_or_else(|| anyhow!("invalid target {s:?} - expected HH:MM, e.g. 4:00 or 6:30"))?;
    let hours: i64 = h
        .parse()
        .with_context(|| format!("invalid hours in target {s:?} - expected HH:MM"))?;
    let minutes: i64 = m
        .parse()
        .with_context(|| format!("invalid minutes in target {s:?} - expected HH:MM"))?;
    if hours < 0 || !(0..60).contains(&minutes) {
        bail!("invalid target {s:?} - hours must be 0 or more and minutes 00-59");
    }
    hours
        .checked_mul(3600)
        .and_then(|seconds| seconds.checked_add(minutes * 60))
        .ok_or_else(|| anyhow!("target {s:?} is too large"))
}

/// Parse `YYYY-MM-DD`, `today` or `yesterday` into a local date.
pub fn parse_date(s: &str) -> Result<NaiveDate> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("today") {
        return Ok(Local::now().date_naive());
    }
    if s.eq_ignore_ascii_case("yesterday") {
        return Local::now()
            .date_naive()
            .pred_opt()
            .ok_or_else(|| anyhow!("yesterday is outside the supported calendar"));
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .with_context(|| format!("invalid date {s:?} - expected YYYY-MM-DD, today or yesterday"))
}

/// Parse a report period: `YYYY`, `YYYY-MM`, `YYYY-Www`, `YYYY-MM-DD`, `today` or `yesterday`.
pub fn parse_period(s: &str) -> Result<Period> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("today") {
        return Ok(Period::Day(Local::now().date_naive()));
    }
    if s.eq_ignore_ascii_case("yesterday") {
        let day = Local::now()
            .date_naive()
            .pred_opt()
            .ok_or_else(|| anyhow!("yesterday is outside the supported calendar"))?;
        return Ok(Period::Day(day));
    }

    let invalid = || {
        anyhow!(
            "invalid period {s:?} - expected YYYY, YYYY-MM, YYYY-Www, YYYY-MM-DD, today or yesterday"
        )
    };
    let year = |y: &str| -> Option<i32> { (y.len() == 4).then(|| y.parse().ok()).flatten() };

    let parts: Vec<&str> = s.split('-').collect();
    match parts.as_slice() {
        [y] => Ok(Period::Year(year(y).ok_or_else(invalid)?)),
        [y, m] if m.starts_with('W') || m.starts_with('w') => {
            let y = year(y).ok_or_else(invalid)?;
            let week: u32 = m[1..].parse().map_err(|_| invalid())?;
            if !(1..=53).contains(&week)
                || NaiveDate::from_isoywd_opt(y, week, chrono::Weekday::Mon).is_none()
            {
                bail!("invalid ISO week in period {s:?} - expected a real week number");
            }
            Ok(Period::Week(y, week))
        }
        [y, m] => {
            let y = year(y).ok_or_else(invalid)?;
            let m: u32 = m.parse().map_err(|_| invalid())?;
            if !(1..=12).contains(&m) {
                bail!("invalid month in period {s:?} - expected 01-12");
            }
            Ok(Period::Month(y, m))
        }
        [_, _, _] => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(Period::Day)
            .map_err(|_| anyhow!("invalid date in period {s:?} - not a real calendar date")),
        _ => Err(invalid()),
    }
}

/// Combine a date and a time into a local timestamp.
pub fn local_datetime(date: NaiveDate, time: NaiveTime) -> Result<DateTime<Local>> {
    Local
        .from_local_datetime(&date.and_time(time))
        .single()
        .with_context(|| format!("ambiguous local time {time}"))
}

/// Ask a `[y/N]` question on the terminal. Enter or end of input means no.
pub fn confirm(question: &str) -> Result<bool> {
    loop {
        print!("\n{question} [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        let read = io::stdin()
            .read_line(&mut input)
            .context("failed to read confirmation")?;
        if read == 0 {
            return Ok(false);
        }

        match input.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "" | "n" | "no" => return Ok(false),
            _ => println!("Please answer yes or no."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_time_and_explicit_datetime() {
        assert!(matches!(parse_at("09:30").unwrap(), At::Time(_)));
        assert!(matches!(
            parse_at("2026-05-28T01:30").unwrap(),
            At::DateTime(_)
        ));
    }

    #[test]
    fn parses_iso_week_periods() {
        assert!(matches!(
            parse_period("2026-W01").unwrap(),
            Period::Week(2026, 1)
        ));
        assert!(parse_period("2026-W54").is_err());
    }

    #[test]
    fn parses_relative_dates() {
        let before = Local::now().date_naive();
        let today = parse_date("today").unwrap();
        let yesterday = parse_date("yesterday").unwrap();
        let after = Local::now().date_naive();
        assert!(today == before || today == after);
        assert!(yesterday == before.pred_opt().unwrap() || yesterday == after.pred_opt().unwrap());
    }

    #[test]
    fn rejects_targets_that_overflow_seconds() {
        assert!(parse_target("999999999999999999:00").is_err());
    }
}
