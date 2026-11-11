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
    let key = format!("{:<12}", format!("{key}:"));
    println!("{}{}", paint(key, Style::Dim), value);
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
        paint(" (running)", Style::Warning)
    } else {
        String::new()
    };
    let duration = (p.ended_at.unwrap_or(now) - p.started_at).num_seconds();
    format!(
        "pause {n}  {}  {}{running}",
        fmt_label(p.label.as_deref()),
        fmt_duration(duration)
    )
}

/// The `Key: value` block shown by `status`, `session end` and `session edit`.
/// `show_ended` lets `session end` skip the Ended line its first line already printed.
pub fn print_session_summary(s: &Session, now: DateTime<Local>, show_ended: bool) {
    let end = s.ended_at.unwrap_or(now);
    let worked = s.worked_secs_at(end);

    kv("Started", fmt_dt(s.started_at));
    match s.ended_at {
        Some(e) if show_ended => kv("Ended", fmt_dt(e)),
        Some(_) => {}
        None => kv("Elapsed", fmt_duration((now - s.started_at).num_seconds())),
    }
    kv("Pauses", fmt_duration(s.pause_secs_at(end)));
    kv("Worked", fmt_duration(worked));
    kv("Target", fmt_target(s.target_secs));
    kv("Overtime", fmt_signed(worked - s.target_secs));
}
