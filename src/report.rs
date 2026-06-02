/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::store::{format_duration, Session, Store};
use crate::util::NORMAL_WORK_SECS;
use anyhow::{bail, Result};
use chrono::{Datelike, IsoWeek, Local};
use clap::{arg, Args, Subcommand};
use std::collections::BTreeMap;

#[derive(Args)]
pub struct Report {
    /// Filter by year, e.g. --year 2026
    #[arg(short, long)]
    year: Option<i32>,
    /// Filter by month (1–12), e.g. --month 5
    #[arg(short, long)]
    month: Option<u32>,
    /// Filter by day (1–31), e.g. --day 22
    #[arg(short, long)]
    day: Option<u32>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show entries for the current day
    Now,
}

pub fn handle(args: &Report) -> Result<()> {
    let filter = match &args.command {
        Some(Commands::Now) => {
            let now = Local::now();

            Filters {
                year: None,
                month: None,
                day: Some(now.day()),
            }
        }

        None => Filters {
            year: args.year,
            month: args.month,
            day: args.day,
        },
    };

    handle_cmd(filter)?;

    Ok(())
}

/// Filters passed in from the terminal.
pub struct Filters {
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub day: Option<u32>,
}

/// Aggregated stats for a group of sessions.
#[derive(Default)]
struct Bucket {
    days: u32,
    worked_secs: i64,
    pause_secs: i64,
}

impl Bucket {
    fn overtime_secs(&self) -> i64 {
        self.worked_secs - (self.days as i64 * NORMAL_WORK_SECS)
    }
}

fn add_session(bucket: &mut Bucket, session: &Session) {
    if let Some(worked) = session.worked_secs() {
        bucket.days += 1;
        bucket.worked_secs += worked;
        bucket.pause_secs += session.total_pause_secs();
    }
}

fn print_bucket(label: &str, b: &Bucket) {
    let overtime = b.overtime_secs();
    let sign = if overtime >= 0 { "+" } else { "-" };
    println!(
        "  {:<18} {:>3} day(s)   worked {}   pauses {}   overtime {}{}",
        label,
        b.days,
        format_duration(b.worked_secs),
        format_duration(b.pause_secs),
        sign,
        format_duration(overtime.abs()),
    );
}

fn handle_cmd(f: Filters) -> Result<()> {
    let now = Local::now();

    // Resolve defaults: --month or --day without --year implies current year.
    let year = f.year.or_else(|| {
        if f.month.is_some() || f.day.is_some() {
            Some(now.year())
        } else {
            None
        }
    });
    // --day without --month implies current month.
    let month = f.month.or_else(|| {
        if f.day.is_some() {
            Some(now.month())
        } else {
            None
        }
    });
    let day = f.day;

    // Validate flag combos.
    if day.is_some() && month.is_none() {
        bail!(
            "--day requires --month (or the current month is implied; pass --month explicitly if needed)"
        );
    }

    let store = Store::load()?;

    // Only completed sessions, filtered by the provided flags.
    let sessions: Vec<&Session> = store
        .sessions
        .iter()
        .filter(|s| !s.is_active())
        .filter(|s| year.map_or(true, |y| s.started_at.year() == y))
        .filter(|s| month.map_or(true, |m| s.started_at.month() == m))
        .filter(|s| day.map_or(true, |d| s.started_at.day() == d))
        .collect();

    if sessions.is_empty() {
        println!("No completed sessions for the given filter.");
        return Ok(());
    }

    match (year, month, day) {
        // No filters → all-time, broken down by year.
        (None, None, None) => {
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

        // Year only → that year broken down by month.
        (Some(y), None, None) => {
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

        // Year + month → that month broken down by ISO week.
        (Some(y), Some(m), None) => {
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

        // Year + month + day → list each session individually.
        (Some(y), Some(m), Some(d)) => {
            println!("── {y}-{m:02}-{d:02} sessions ──────────────────────────────────────");
            let mut totals = Bucket::default();
            for s in &sessions {
                let started = s.started_at.format("%H:%M:%S");
                let ended = s
                    .ended_at
                    .map(|e| e.format("%H:%M:%S").to_string())
                    .unwrap_or_else(|| "ongoing".to_string());
                let worked = s.worked_secs().unwrap_or(0);
                let pause = s.total_pause_secs();
                let ot = worked - NORMAL_WORK_SECS;
                let sign = if ot >= 0 { "+" } else { "-" };
                println!(
                    "  {} → {}   worked {}   pauses {}   overtime {}{}",
                    started,
                    ended,
                    format_duration(worked),
                    format_duration(pause),
                    sign,
                    format_duration(ot.abs()),
                );

                let mut i = 0;
                for p in &s.pauses {
                    if let Some(dur) = p.duration_secs() {
                        let label = p.label.as_deref().unwrap_or("(no label)");
                        println!("  [{}] {} - {}", i + 1, label, format_duration(dur));
                        i += 1;
                    }
                }
                add_session(&mut totals, s);
            }
            println!("{}", "─".repeat(61));
            print_bucket(&format!("{y}-{m:02}-{d:02} total"), &totals);
        }

        // --month without --year is resolved above; this arm is unreachable.
        _ => unreachable!(),
    }

    println!();
    Ok(())
}
