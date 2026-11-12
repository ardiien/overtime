/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

//! Every user-facing formatter lives here so all commands print the same way.

use crate::store::{Pause, Session};
use anstyle::{AnsiColor, RgbColor, Style as AnsiStyle};
use chrono::{DateTime, Local};
use std::fmt::Display;
use std::sync::atomic::{AtomicBool, Ordering};

static COLOR_ENABLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
pub enum Style {
    Error,
    Good,
    Bad,
    Warning,
    Dim,
    Bold,
}

pub fn set_color_enabled(enabled: bool) {
    COLOR_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn paint(text: impl Display, style: Style) -> String {
    if !COLOR_ENABLED.load(Ordering::Relaxed) {
        return text.to_string();
    }

    let style = match style {
        Style::Error => AnsiStyle::new()
            .fg_color(Some(AnsiColor::Red.into()))
            .bold(),
        Style::Good => AnsiStyle::new().fg_color(Some(AnsiColor::Green.into())),
        Style::Bad => AnsiStyle::new().fg_color(Some(RgbColor(255, 165, 0).into())),
        Style::Warning => AnsiStyle::new().fg_color(Some(AnsiColor::Yellow.into())),
        Style::Dim => AnsiStyle::new().dimmed(),
        Style::Bold => AnsiStyle::new().bold(),
    };
    format!("{style}{text}{style:#}")
}

/// `2026-05-27 09:00:00`
pub fn fmt_dt(dt: DateTime<Local>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// `09:00:00`
pub fn fmt_time(dt: DateTime<Local>) -> String {
    dt.format("%H:%M:%S").to_string()
}

/// `08:30:00`
pub fn fmt_duration(total_secs: i64) -> String {
    let t = total_secs.unsigned_abs();
    format!("{:02}:{:02}:{:02}", t / 3600, (t % 3600) / 60, t % 60)
}

/// Always signed: `+00:30:00` / `-01:15:00`.
pub fn fmt_signed(total_secs: i64) -> String {
    let sign = if total_secs < 0 { "-" } else { "+" };
    let value = format!("{sign}{}", fmt_duration(total_secs));
    paint(
        value,
        if total_secs < 0 {
            Style::Bad
        } else {
            Style::Good
        },
    )
}

/// Daily target without seconds: `08:00`.
pub fn fmt_target(total_secs: i64) -> String {
    format!("{:02}:{:02}", total_secs / 3600, (total_secs % 3600) / 60)
}

/// Pause label: `[lunch]`, or `[no label]` when unset.
pub fn fmt_label(label: Option<&str>) -> String {
    paint(
        match label {
            Some(l) => format!("[{l}]"),
            None => "[no label]".to_string(),
        },
        Style::Dim,
    )
}

/// One aligned `Key: value` line.
pub fn kv(key: &str, value: impl Display) {
    print_kv(key, value, "", ":");
}

/// One aligned, indented key/value line without punctuation after the key.
pub fn kv_indented_no_colon(key: &str, value: impl Display) {
    print_kv(key, value, "  ", "");
}

fn print_kv(key: &str, value: impl Display, key_prefix: &str, key_suffix: &str) {
    println!("{}", format_kv(key, value, key_prefix, key_suffix));
}

fn format_kv(key: &str, value: impl Display, key_prefix: &str, key_suffix: &str) -> String {
    let key = format!("{key}{key_suffix}");
    let width = 12.max(key.len() + 1);
    let key = format!("{key:<width$}");
    format!("{key_prefix}{}{}", paint(key, Style::Dim), value)
}

/// One-line session description shared by `report <DATE>` and `session delete`.
pub fn fmt_session_row(s: &Session, now: DateTime<Local>) -> String {
    let end = s.ended_at.unwrap_or(now);
    let ended = s
        .ended_at
        .map(fmt_time)
        .unwrap_or_else(|| "ongoing".to_string());
    let ended = format!("{ended:<8}");
    let ended = if s.ended_at.is_some() {
        ended
    } else {
        paint(ended, Style::Warning)
    };
    let worked = s.worked_secs_at(end);
    format!(
        "{} → {}   target {}   worked {}   pauses {}   overtime {}",
        fmt_time(s.started_at),
        ended,
        fmt_target(s.target_secs),
        fmt_duration(worked),
        fmt_duration(s.pause_secs_at(end)),
        fmt_signed(worked - s.target_secs),
    )
}

/// One-line pause description shared by `report <DATE>` and `pause delete`.
/// `n` is the pause's 1-based number within its session.
pub fn fmt_pause_row(n: usize, p: &Pause, now: DateTime<Local>) -> String {
    let running = if p.is_active() {
        paint("ongoing", Style::Warning)
    } else {
        String::new()
    };
    let duration = (p.ended_at.unwrap_or(now) - p.started_at).num_seconds();
    let ended = p.ended_at.map(fmt_time).unwrap_or_else(|| running);
    format!(
        "pause {n}  {}  {} → {} | {}",
        fmt_label(p.label.as_deref()),
        fmt_time(p.started_at),
        ended,
        fmt_duration(duration)
    )
}

/// The key/value block shown by `session end` and `session edit`.
/// `show_ended` lets `session end` skip the Ended line its first line already printed.
/// `key_prefix` indents each item; `key_suffix` is appended to each item name.
pub fn print_session_summary(
    s: &Session,
    now: DateTime<Local>,
    show_ended: bool,
    key_prefix: &str,
    key_suffix: &str,
) {
    let end = s.ended_at.unwrap_or(now);
    let worked = s.worked_secs_at(end);

    print_kv("Started", fmt_dt(s.started_at), key_prefix, key_suffix);
    match s.ended_at {
        Some(e) if show_ended => print_kv("Ended", fmt_dt(e), key_prefix, key_suffix),
        Some(_) => {}
        None => print_kv(
            "Elapsed",
            fmt_duration((now - s.started_at).num_seconds()),
            key_prefix,
            key_suffix,
        ),
    }
    print_kv(
        "Pauses",
        fmt_duration(s.pause_secs_at(end)),
        key_prefix,
        key_suffix,
    );
    print_kv("Worked", fmt_duration(worked), key_prefix, key_suffix);
    print_kv("Target", fmt_target(s.target_secs), key_prefix, key_suffix);
    print_kv(
        "Overtime",
        fmt_signed(worked - s.target_secs),
        key_prefix,
        key_suffix,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveTime, TimeZone};

    fn dt(hour: u32, minute: u32) -> DateTime<Local> {
        Local
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2026, 5, 27)
                    .unwrap()
                    .and_time(NaiveTime::from_hms_opt(hour, minute, 0).unwrap()),
            )
            .single()
            .unwrap()
    }

    #[test]
    fn pause_row_shows_start_and_end_times() {
        set_color_enabled(false);
        let pause = Pause {
            started_at: dt(12, 0),
            ended_at: Some(dt(12, 30)),
            label: Some("lunch".to_string()),
        };

        assert_eq!(
            fmt_pause_row(1, &pause, dt(13, 0)),
            "pause 1  [lunch]  12:00:00 → 12:30:00 | 00:30:00"
        );
    }

    #[test]
    fn active_pause_row_shows_ongoing_end_time() {
        set_color_enabled(false);
        let pause = Pause {
            started_at: dt(12, 0),
            ended_at: None,
            label: None,
        };

        assert_eq!(
            fmt_pause_row(1, &pause, dt(12, 5)),
            "pause 1  [no label]  12:00:00 → ongoing | 00:05:00"
        );
    }

    #[test]
    fn key_value_line_can_omit_colon() {
        set_color_enabled(false);
        assert_eq!(format_kv("Progress", "42%", "  ", ""), "  Progress    42%");
        assert_eq!(
            format_kv("End ETA", "17:31:03", "  ", ""),
            "  End ETA     17:31:03"
        );
    }
}
