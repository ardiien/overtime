/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use chrono::{NaiveDate, NaiveTime};

/// Default daily target (8 hours) in seconds. Used when no `--target` is given
/// and for sessions stored before per-session targets existed.
pub const DEFAULT_TARGET_SECS: i64 = 8 * 3600;

/// Tool name for printing.
pub const TOOL_NAME: &str = "zeit";

/// Parse "HH:MM" or "HH:MM:SS" into a NaiveTime.
pub fn parse_time(s: &str) -> anyhow::Result<NaiveTime> {
    use anyhow::Context;
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
        .with_context(|| format!("invalid time {s:?} — expected HH:MM or HH:MM:SS"))
}

/// Parse a daily target given as "HH:MM" (e.g. "4:00", "04:30", "8:00") into seconds.
pub fn parse_target(s: &str) -> anyhow::Result<i64> {
    use anyhow::{anyhow, Context};
    let (h, m) = s
        .split_once(':')
        .ok_or_else(|| anyhow!("invalid target {s:?} — expected HH:MM, e.g. 4:00 or 6:30"))?;
    let hours: i64 = h
        .parse()
        .with_context(|| format!("invalid hours in target {s:?} — expected HH:MM"))?;
    let minutes: i64 = m
        .parse()
        .with_context(|| format!("invalid minutes in target {s:?} — expected HH:MM"))?;
    if hours < 0 || !(0..60).contains(&minutes) {
        return Err(anyhow!(
            "invalid target {s:?} — hours must be >= 0 and minutes 0-59"
        ));
    }
    Ok(hours * 3600 + minutes * 60)
}

/// Format seconds as "HH:MM" (used to display a daily target).
pub fn format_hhmm(total_secs: i64) -> String {
    format!("{:02}:{:02}", total_secs / 3600, (total_secs % 3600) / 60)
}

/// Parse "YYYY-MM-DD" into a NaiveDate.
pub fn parse_date(s: &str) -> anyhow::Result<NaiveDate> {
    use anyhow::Context;
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .with_context(|| format!("invalid date {s:?} — expected YYYY-MM-DD"))
}
