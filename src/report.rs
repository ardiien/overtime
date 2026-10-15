/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::output::{fmt_duration, fmt_pause_row, fmt_session_row, fmt_signed};
use crate::store::{Session, Store};
use crate::util::{Period, parse_period};
use anyhow::Result;
use chrono::{Datelike, IsoWeek, Local};
use clap::Args;
use std::collections::BTreeMap;

#[derive(Args)]
pub struct Report {
    /// Period to report on: YYYY, YYYY-MM, YYYY-MM-DD or `today`.
    /// Omit for the all-time summary, broken down by year.
    #[arg(value_parser = parse_period)]
    period: Option<Period>,
}

/// Aggregated stats for a group of sessions.
#[derive(Default)]
struct Bucket {
    days: u32,
    worked_secs: i64,
    pause_secs: i64,
    /// Sum of each session's own daily target.
    target_secs: i64,
}

impl Bucket {
    fn overtime_secs(&self) -> i64 {
        self.worked_secs - self.target_secs
    }
}

fn add_session(bucket: &mut Bucket, session: &Session) {
    if let (Some(worked), Some(ended)) = (session.worked_secs(), session.ended_at) {
        bucket.days += 1;
        bucket.worked_secs += worked;
        bucket.pause_secs += session.pause_secs_at(ended);
        bucket.target_secs += session.target_secs;
    }
}

fn print_bucket(label: &str, b: &Bucket) {
    println!(
        "  {:<18} {:>3} day(s)   worked {}   pauses {}   overtime {}",
        label,
        b.days,
        fmt_duration(b.worked_secs),
        fmt_duration(b.pause_secs),
        fmt_signed(b.overtime_secs()),
    );
}

pub fn handle(args: &Report) -> Result<()> {
    let store = Store::load()?;

    // Only completed sessions, filtered by the requested period.
    let sessions: Vec<&Session> = store
        .sessions
        .iter()
        .filter(|s| !s.is_active())
        .filter(|s| match args.period {
            None => true,
            Some(Period::Year(y)) => s.started_at.year() == y,
            Some(Period::Month(y, m)) => s.started_at.year() == y && s.started_at.month() == m,
            Some(Period::Day(d)) => s.started_at.date_naive() == d,
        })
        .collect();

    if sessions.is_empty() {
        match args.period {
            None => println!("No completed sessions."),
            Some(p) => println!("No completed sessions for {}.", p.label()),
        }
        return Ok(());
    }

    match args.period {
        // No period → all-time, broken down by year.
        None => {
            println!("── All-time report (by year) ────────────────────────────────");
            let mut totals = Bucket::default();
            let mut by_year: BTreeMap<i32, Bucket> = BTreeMap::new();
            for s in &sessions {
                add_session(by_year.entry(s.started_at.year()).or_default(), s);
                add_session(&mut totals, s);
            }
            for (y, b) in &by_year {
                print_bucket(&y.to_string(), b);
            }
            println!("{}", "─".repeat(61));
            print_bucket("Total", &totals);
        }

        // Year → that year broken down by month.
        Some(Period::Year(y)) => {
            println!("── {y} report (by month) ────────────────────────────────────");
            let mut totals = Bucket::default();
            let mut by_month: BTreeMap<u32, Bucket> = BTreeMap::new();
            for s in &sessions {
                add_session(by_month.entry(s.started_at.month()).or_default(), s);
                add_session(&mut totals, s);
            }
            for (m, b) in &by_month {
                print_bucket(&format!("{y}-{m:02}"), b);
            }
            println!("{}", "─".repeat(61));
            print_bucket(&format!("{y} total"), &totals);
        }

        // Month → that month broken down by ISO week.
        Some(Period::Month(y, m)) => {
            println!("── {y}-{m:02} report (by week) ──────────────────────────────────");
            let mut totals = Bucket::default();
            let mut by_week: BTreeMap<(i32, u32), Bucket> = BTreeMap::new();
            for s in &sessions {
                let iso: IsoWeek = s.started_at.iso_week();
                add_session(by_week.entry((iso.year(), iso.week())).or_default(), s);
                add_session(&mut totals, s);
            }
            for ((wy, wn), b) in &by_week {
                print_bucket(&format!("{wy}-W{wn:02}"), b);
            }
            println!("{}", "─".repeat(61));
            print_bucket(&format!("{y}-{m:02} total"), &totals);
        }

        // Day → list each session individually, with its pauses.
        Some(Period::Day(d)) => {
            println!("── {d} sessions ──────────────────────────────────────────");
            let now = Local::now();
            let mut totals = Bucket::default();
            for s in &sessions {
                println!("  {}", fmt_session_row(s, now));
                for (i, p) in s.pauses.iter().enumerate() {
                    println!("    {}", fmt_pause_row(i + 1, p, now));
                }
                add_session(&mut totals, s);
            }
            println!("{}", "─".repeat(61));
            print_bucket(&format!("{d} total"), &totals);
        }
    }

    println!();
    Ok(())
}
