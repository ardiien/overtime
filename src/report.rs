/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::output::{
    Style, fmt_duration, fmt_pause_label_width, fmt_pause_row_aligned, fmt_session_row,
    fmt_signed_aligned, paint,
};
use crate::store::{Session, Store};
use crate::util::{Period, parse_period};
use anyhow::Result;
use chrono::{Datelike, IsoWeek, Local, NaiveDate};
use clap::Args;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Args)]
pub struct Report {
    /// Period to report on: YYYY, YYYY-MM, YYYY-Www, YYYY-MM-DD, today or yesterday.
    /// Omit for the all-time summary, broken down by year.
    #[arg(value_parser = parse_period)]
    period: Option<Period>,
    /// Export the report as JSON.
    #[arg(long, conflicts_with = "csv")]
    json: bool,
    /// Export the report as CSV.
    #[arg(long)]
    csv: bool,
}

#[derive(Default)]
struct Bucket {
    dates: BTreeSet<NaiveDate>,
    sessions: u32,
    ongoing_sessions: u32,
    pause_count: u32,
    worked_secs: i64,
    pause_secs: i64,
    target_secs: i64,
}

impl Bucket {
    fn overtime_secs(&self) -> i64 {
        self.worked_secs - self.target_secs
    }

    fn line(&self, period: impl Into<String>) -> ReportLine {
        ReportLine {
            period: period.into(),
            days: self.dates.len() as u32,
            sessions: self.sessions,
            ongoing_sessions: self.ongoing_sessions,
            pause_count: self.pause_count,
            worked_seconds: self.worked_secs,
            pause_seconds: self.pause_secs,
            target_seconds: self.target_secs,
            overtime_seconds: self.overtime_secs(),
        }
    }
}

#[derive(Serialize)]
struct ReportLine {
    period: String,
    days: u32,
    sessions: u32,
    ongoing_sessions: u32,
    pause_count: u32,
    worked_seconds: i64,
    pause_seconds: i64,
    target_seconds: i64,
    overtime_seconds: i64,
}

#[derive(Serialize)]
struct JsonReport<'a> {
    period: String,
    rows: &'a [ReportLine],
    total: &'a ReportLine,
}

struct ReportLineWidths {
    period: usize,
    days: usize,
    worked: usize,
    pause_count: usize,
    pause_seconds: usize,
    overtime: usize,
}

impl ReportLineWidths {
    fn for_lines<'a>(lines: impl Iterator<Item = &'a ReportLine>) -> Self {
        let mut widths = Self {
            period: 18,
            days: 3,
            worked: 8,
            pause_count: 1,
            pause_seconds: 8,
            overtime: 9,
        };
        for line in lines {
            widths.period = widths.period.max(line.period.chars().count());
            widths.days = widths.days.max(line.days.to_string().len());
            widths.worked = widths.worked.max(fmt_duration(line.worked_seconds).len());
            widths.pause_count = widths.pause_count.max(line.pause_count.to_string().len());
            widths.pause_seconds = widths
                .pause_seconds
                .max(fmt_duration(line.pause_seconds).len());
            widths.overtime = widths
                .overtime
                .max(fmt_duration(line.overtime_seconds).len() + 1);
        }
        widths
    }
}

fn add_session(bucket: &mut Bucket, session: &Session, now: chrono::DateTime<Local>) {
    let end = session.ended_at.unwrap_or(now);
    bucket.dates.insert(session.started_at.date_naive());
    bucket.sessions += 1;
    bucket.ongoing_sessions += u32::from(session.is_active());
    bucket.pause_count += session.pauses.len() as u32;
    bucket.worked_secs += session
        .worked_secs()
        .unwrap_or_else(|| session.worked_secs_at(end));
    bucket.pause_secs += session.pause_secs_at(end);
    bucket.target_secs += session.target_secs;
}

fn format_line(line: &ReportLine, widths: &ReportLineWidths) -> String {
    let is_total = line.period == "Total" || line.period.ends_with(" total");
    let padded_label = format!("{:<width$}", line.period, width = widths.period);
    let label = if is_total {
        paint(padded_label, Style::Bold)
    } else {
        padded_label
    };
    let ongoing = if !is_total && line.ongoing_sessions > 0 {
        paint(
            format!("   ongoing {}", line.ongoing_sessions),
            Style::Warning,
        )
    } else {
        String::new()
    };
    format!(
        "  {label} {days:>days_width$} day(s)   worked {worked:>worked_width$}   pauses {pause_count:>pause_count_width$} ({pause_seconds:>pause_seconds_width$})   overtime {overtime}{ongoing}",
        days = line.days,
        days_width = widths.days,
        worked = fmt_duration(line.worked_seconds),
        worked_width = widths.worked,
        pause_count = line.pause_count,
        pause_count_width = widths.pause_count,
        pause_seconds = fmt_duration(line.pause_seconds),
        pause_seconds_width = widths.pause_seconds,
        overtime = fmt_signed_aligned(line.overtime_seconds, widths.overtime),
    )
}

fn print_line(line: &ReportLine, widths: &ReportLineWidths) {
    println!("{}", format_line(line, widths));
}

fn export_json(period: &str, rows: &[ReportLine], total: &ReportLine) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&JsonReport {
            period: period.to_string(),
            rows,
            total,
        })?
    );
    Ok(())
}

fn export_csv(rows: &[ReportLine], total: &ReportLine) {
    println!(
        "period,days,sessions,ongoing_sessions,pause_count,worked_seconds,pause_seconds,target_seconds,overtime_seconds"
    );
    for row in rows.iter().chain(std::iter::once(total)) {
        println!(
            "{},{},{},{},{},{},{},{},{}",
            row.period,
            row.days,
            row.sessions,
            row.ongoing_sessions,
            row.pause_count,
            row.worked_seconds,
            row.pause_seconds,
            row.target_seconds,
            row.overtime_seconds
        );
    }
}

pub fn handle(args: &Report) -> Result<()> {
    let store = Store::load()?;
    let now = Local::now();

    let sessions: Vec<&Session> = store
        .sessions
        .iter()
        .filter(|s| match args.period {
            None => true,
            Some(Period::Year(y)) => s.started_at.year() == y,
            Some(Period::Month(y, m)) => s.started_at.year() == y && s.started_at.month() == m,
            Some(Period::Week(y, w)) => {
                let iso = s.started_at.iso_week();
                iso.year() == y && iso.week() == w
            }
            Some(Period::Day(d)) => s.started_at.date_naive() == d,
        })
        .collect();

    if sessions.is_empty() {
        if args.json {
            return export_json(
                &args.period.map_or_else(|| "all".to_string(), |p| p.label()),
                &[],
                &Bucket::default().line("Total"),
            );
        }
        if args.csv {
            export_csv(&[], &Bucket::default().line("Total"));
            return Ok(());
        }
        match args.period {
            None => println!("No sessions."),
            Some(p) => println!("No sessions for {}.", p.label()),
        }
        return Ok(());
    }

    let mut total = Bucket::default();
    for session in &sessions {
        add_session(&mut total, session, now);
    }

    let mut rows = Vec::new();
    match args.period {
        None => {
            let mut by_year: BTreeMap<i32, Bucket> = BTreeMap::new();
            for session in &sessions {
                add_session(
                    by_year.entry(session.started_at.year()).or_default(),
                    session,
                    now,
                );
            }
            rows.extend(
                by_year
                    .iter()
                    .map(|(year, bucket)| bucket.line(year.to_string())),
            );
        }
        Some(Period::Year(year)) => {
            let mut by_month: BTreeMap<u32, Bucket> = BTreeMap::new();
            for session in &sessions {
                add_session(
                    by_month.entry(session.started_at.month()).or_default(),
                    session,
                    now,
                );
            }
            rows.extend(
                by_month
                    .iter()
                    .map(|(month, bucket)| bucket.line(format!("{year}-{month:02}"))),
            );
        }
        Some(Period::Month(_, _)) => {
            let mut by_week: BTreeMap<(i32, u32), Bucket> = BTreeMap::new();
            for session in &sessions {
                let iso: IsoWeek = session.started_at.iso_week();
                add_session(
                    by_week.entry((iso.year(), iso.week())).or_default(),
                    session,
                    now,
                );
            }
            rows.extend(
                by_week.iter().map(|((week_year, week), bucket)| {
                    bucket.line(format!("{week_year}-W{week:02}"))
                }),
            );
        }
        Some(Period::Week(_, _)) => {
            let mut by_day: BTreeMap<NaiveDate, Bucket> = BTreeMap::new();
            for session in &sessions {
                add_session(
                    by_day.entry(session.started_at.date_naive()).or_default(),
                    session,
                    now,
                );
            }
            rows.extend(
                by_day
                    .iter()
                    .map(|(date, bucket)| bucket.line(date.to_string())),
            );
        }
        Some(Period::Day(date)) => rows.push(total.line(date.to_string())),
    }

    let total_period = match args.period {
        None => "Total".to_string(),
        Some(Period::Year(year)) => format!("{year} total"),
        Some(Period::Month(year, month)) => format!("{year}-{month:02} total"),
        Some(Period::Week(year, week)) => format!("{year}-W{week:02} total"),
        Some(Period::Day(date)) => format!("{date} total"),
    };
    let total_line = total.line(total_period);
    if args.json {
        return export_json(
            &args.period.map_or_else(|| "all".to_string(), |p| p.label()),
            &rows,
            &total_line,
        );
    }
    if args.csv {
        export_csv(&rows, &total_line);
        return Ok(());
    }
    let widths = ReportLineWidths::for_lines(rows.iter().chain(std::iter::once(&total_line)));

    match args.period {
        None => {
            println!(
                "{}",
                paint(
                    "── All-time report (by year) ────────────────────────────────",
                    Style::Bold
                )
            );
            for row in &rows {
                print_line(row, &widths);
            }
            println!("{}", "─".repeat(61));
            print_line(&total_line, &widths);
        }
        Some(Period::Year(year)) => {
            println!(
                "{}",
                paint(
                    format!("── {year} report (by month) ────────────────────────────────────"),
                    Style::Bold
                )
            );
            for row in &rows {
                print_line(row, &widths);
            }
            println!("{}", "─".repeat(61));
            print_line(&total_line, &widths);
        }
        Some(Period::Month(year, month)) => {
            println!(
                "{}",
                paint(
                    format!(
                        "── {year}-{month:02} report (by week) ──────────────────────────────────"
                    ),
                    Style::Bold
                )
            );
            for row in &rows {
                print_line(row, &widths);
            }
            println!("{}", "─".repeat(61));
            print_line(&total_line, &widths);
        }
        Some(Period::Week(year, week)) => {
            println!(
                "{}",
                paint(
                    format!(
                        "── {year}-W{week:02} report (by day) ───────────────────────────────────"
                    ),
                    Style::Bold
                )
            );
            for row in &rows {
                print_line(row, &widths);
            }
            println!("{}", "─".repeat(61));
            print_line(&total_line, &widths);
        }
        Some(Period::Day(date)) => {
            println!(
                "{}",
                paint(
                    format!("── {date} sessions ──────────────────────────────────────────"),
                    Style::Bold
                )
            );
            let pause_label_width = sessions
                .iter()
                .flat_map(|session| session.pauses.iter())
                .map(fmt_pause_label_width)
                .max()
                .unwrap_or_default();
            for session in &sessions {
                println!("  {}", fmt_session_row(session, now));
                for (i, pause) in session.pauses.iter().enumerate() {
                    println!(
                        "    {}",
                        fmt_pause_row_aligned(i + 1, pause, now, pause_label_width)
                    );
                }
            }
            println!("{}", "─".repeat(61));
            print_line(&total_line, &widths);
        }
    }

    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveTime, TimeZone};

    fn dt(day: u32, hour: u32) -> chrono::DateTime<Local> {
        Local
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2026, 5, day)
                    .unwrap()
                    .and_time(NaiveTime::from_hms_opt(hour, 0, 0).unwrap()),
            )
            .single()
            .unwrap()
    }

    #[test]
    fn bucket_counts_unique_days_not_sessions() {
        let session = |id, start, end| Session {
            id,
            started_at: dt(start, 9),
            ended_at: Some(dt(start, end)),
            pauses: vec![],
            target_secs: 8 * 3600,
        };
        let now = dt(2, 17);
        let mut bucket = Bucket::default();
        add_session(&mut bucket, &session(1, 1, 12), now);
        add_session(&mut bucket, &session(2, 1, 17), now);
        add_session(
            &mut bucket,
            &Session {
                id: 3,
                started_at: dt(2, 9),
                ended_at: None,
                pauses: vec![],
                target_secs: 8 * 3600,
            },
            now,
        );
        assert_eq!(bucket.dates.len(), 2);
        assert_eq!(bucket.sessions, 3);
        assert_eq!(bucket.ongoing_sessions, 1);
    }

    #[test]
    fn bucket_counts_pauses_across_sessions() {
        let session = |id, pauses| Session {
            id,
            started_at: dt(1, 9),
            ended_at: Some(dt(1, 17)),
            pauses,
            target_secs: 8 * 3600,
        };
        let pause = |start, end| crate::store::Pause {
            started_at: dt(1, start),
            ended_at: Some(dt(1, end)),
            label: None,
        };
        let now = dt(2, 17);
        let mut bucket = Bucket::default();
        add_session(
            &mut bucket,
            &session(1, vec![pause(11, 12), pause(13, 14)]),
            now,
        );
        add_session(&mut bucket, &session(2, vec![pause(12, 13)]), now);

        assert_eq!(bucket.line("2026").pause_count, 3);
    }

    #[test]
    fn total_line_omits_ongoing_count() {
        let mut bucket = Bucket::default();
        bucket.ongoing_sessions = 1;

        let total = bucket.line("2026-11-12 total");
        let row = bucket.line("2026-11-12");
        let widths = ReportLineWidths::for_lines([&total, &row].into_iter());

        assert!(!format_line(&total, &widths).contains("ongoing"));
        assert!(format_line(&row, &widths).contains("ongoing 1"));
    }

    #[test]
    fn report_columns_align_when_totals_have_more_digits() {
        let row = ReportLine {
            period: "2026-W40".to_string(),
            days: 2,
            sessions: 2,
            ongoing_sessions: 0,
            pause_count: 1,
            worked_seconds: 16 * 3600 + 4 * 60 + 41,
            pause_seconds: 36 * 60 + 57,
            target_seconds: 16 * 3600,
            overtime_seconds: 4 * 3600 + 41 * 60,
        };
        let total = ReportLine {
            period: "2026-10 total".to_string(),
            days: 22,
            sessions: 22,
            ongoing_sessions: 0,
            pause_count: 20,
            worked_seconds: 177 * 3600 + 39 * 60 + 50,
            pause_seconds: 16 * 3600 + 2 * 60 + 55,
            target_seconds: 176 * 3600,
            overtime_seconds: 1 * 3600 + 39 * 60 + 50,
        };
        let widths = ReportLineWidths::for_lines([&row, &total].into_iter());
        let row = format_line(&row, &widths);
        let total = format_line(&total, &widths);

        for column in ["day(s)", "worked", "pauses", "overtime"] {
            assert_eq!(row.find(column), total.find(column), "{column} column");
        }

        let pause_duration_column = |line: &str| {
            let pauses = line.find("   pauses ").unwrap();
            pauses + line[pauses..].find('(').unwrap()
        };
        assert_eq!(pause_duration_column(&row), pause_duration_column(&total));
    }
}
