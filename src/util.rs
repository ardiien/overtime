/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Local, NaiveDate, NaiveTime, TimeZone};
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
    Day(NaiveDate),
}

impl Period {
    /// Human-readable form: `2026`, `2026-05` or `2026-05-27`.
    pub fn label(&self) -> String {
        match self {
            Period::Year(y) => format!("{y}"),
            Period::Month(y, m) => format!("{y}-{m:02}"),
            Period::Day(d) => d.to_string(),
        }
    }
}

/// Parse "HH:MM" or "HH:MM:SS" into a NaiveTime.
pub fn parse_time(s: &str) -> Result<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
        .with_context(|| format!("invalid time {s:?} - expected HH:MM or HH:MM:SS"))
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
    Ok(hours * 3600 + minutes * 60)
}

/// Parse "YYYY-MM-DD" into a NaiveDate.
pub fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .with_context(|| format!("invalid date {s:?} - expected YYYY-MM-DD"))
}

/// Parse a report period: `YYYY`, `YYYY-MM`, `YYYY-MM-DD` or `today`.
pub fn parse_period(s: &str) -> Result<Period> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("today") {
        return Ok(Period::Day(Local::now().date_naive()));
    }

    let invalid = || anyhow!("invalid period {s:?} - expected YYYY, YYYY-MM, YYYY-MM-DD or today");
    let year = |y: &str| -> Option<i32> { (y.len() == 4).then(|| y.parse().ok()).flatten() };

    let parts: Vec<&str> = s.split('-').collect();
    match parts.as_slice() {
        [y] => Ok(Period::Year(year(y).ok_or_else(invalid)?)),
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
