/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use chrono::{NaiveDate, NaiveTime};

/// Normal working 8 hours per day in seconds.
pub const NORMAL_WORK_SECS: i64 = 8 * 3600;

/// Tool name for printing.
pub const TOOL_NAME: &str = "zeit";

/// Parse "HH:MM" or "HH:MM:SS" into a NaiveTime.
pub fn parse_time(s: &str) -> anyhow::Result<NaiveTime> {
    use anyhow::Context;
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
        .with_context(|| format!("invalid time {s:?} — expected HH:MM or HH:MM:SS"))
}

/// Parse "YYYY-MM-DD" into a NaiveDate.
pub fn parse_date(s: &str) -> anyhow::Result<NaiveDate> {
    use anyhow::Context;
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .with_context(|| format!("invalid date {s:?} — expected YYYY-MM-DD"))
}
