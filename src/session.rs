/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::output::{fmt_dt, fmt_session_row, fmt_target, fmt_time, kv, print_session_summary};
use crate::store::Store;
use crate::util::{
    local_datetime, parse_date, parse_target, parse_time, DEFAULT_TARGET_SECS, TOOL_NAME,
};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{Local, NaiveDate, NaiveTime};
use clap::{Args, Subcommand};
use std::io::{self, Write};

#[derive(Args)]
pub struct Session {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a new work session.
    Start {
        /// Start at a specific past time today, e.g. --at 09:00.
        #[arg(short, long, value_parser = parse_time)]
        at: Option<NaiveTime>,
        /// Daily target for this session as HH:MM, e.g. --target 4:00 (default 8:00).
        #[arg(short, long, value_parser = parse_target)]
        target: Option<i64>,
    },
    /// End the current work session.
    End {
        /// End at a specific past time today, e.g. --at 17:30.
        #[arg(short, long, value_parser = parse_time)]
        at: Option<NaiveTime>,
    },
    /// Edit a session's start time, end time or daily target.
    Edit {
        /// Date of the session to edit (YYYY-MM-DD).
        #[arg(value_parser = parse_date)]
        date: NaiveDate,
        /// New start time (HH:MM or HH:MM:SS).
        #[arg(short, long, value_parser = parse_time)]
        start: Option<NaiveTime>,
        /// New end time (HH:MM or HH:MM:SS).
        #[arg(short, long, value_parser = parse_time)]
        end: Option<NaiveTime>,
        /// New daily target as HH:MM, e.g. --target 4:00.
        #[arg(short, long, value_parser = parse_target)]
        target: Option<i64>,
    },
    /// Delete all sessions on a date.
    Delete {
        /// Date to delete (YYYY-MM-DD).
        #[arg(value_parser = parse_date)]
        date: NaiveDate,
        /// Skip the confirmation prompt.
        #[arg(short, long)]
        yes: bool,
    },
}

pub fn handle(args: &Session) -> Result<()> {
    match &args.command {
        Commands::Start { at, target } => start(*at, *target),
        Commands::End { at } => end(*at),
        Commands::Edit {
            date,
            start,
            end,
            target,
        } => edit(*date, *start, *end, *target),
        Commands::Delete { date, yes } => delete(*date, *yes),
    }
}

fn start(at: Option<NaiveTime>, target: Option<i64>) -> Result<()> {
    let mut store = Store::load()?;

    if store.active_session_mut().is_some() {
        bail!("there is already an active session - run `{TOOL_NAME} session end` first.");
    }

    let now = Local::now();
    let started_at = match at {
        Some(t) => local_datetime(now.date_naive(), t)?,
        None => now,
    };
    if started_at > now {
        bail!(
            "--at {} is in the future - use a time up to now.",
            fmt_time(started_at)
        );
    }

    let target_secs = target.unwrap_or(DEFAULT_TARGET_SECS);
    store
        .sessions
        .push(crate::store::Session::new(started_at, target_secs));
    store.save()?;

    println!("Started session at {}", fmt_dt(started_at));
    kv("Target", fmt_target(target_secs));
    Ok(())
}

fn end(at: Option<NaiveTime>) -> Result<()> {
    let mut store = Store::load()?;

    let session = store
        .active_session_mut()
        .ok_or_else(|| anyhow!("no active session - run `{TOOL_NAME} session start` first."))?;

    if session.has_active_pause() {
        bail!("a pause is still running - run `{TOOL_NAME} pause end` first.");
    }

    let now = Local::now();
    let ended_at = match at {
        Some(t) => local_datetime(session.started_at.date_naive(), t)?,
        None => now,
    };
    if ended_at <= session.started_at {
        bail!(
            "--at {} is not after session start ({}).",
            fmt_time(ended_at),
            fmt_time(session.started_at)
        );
    }
    if ended_at > now {
        bail!(
            "--at {} is in the future - use a time up to now.",
            fmt_time(ended_at)
        );
    }

    session.ended_at = Some(ended_at);
    let finished = session.clone();
    store.save()?;

    println!("Ended session at {}", fmt_dt(ended_at));
    print_session_summary(&finished, now, false);
    Ok(())
}

fn edit(
    date: NaiveDate,
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
    target: Option<i64>,
) -> Result<()> {
    if start.is_none() && end.is_none() && target.is_none() {
        bail!("nothing to edit - provide --start, --end or --target.");
    }

    let mut store = Store::load()?;
    let idx = *store
        .session_indices_on(date)
        .last()
        .ok_or_else(|| anyhow!("no session found for {date}."))?;
    let session = &mut store.sessions[idx];

    // Work out and validate every new value first, so a failed edit prints and saves nothing.
    let old_start = session.started_at;
    let old_end = session.ended_at;
    let old_target = session.target_secs;

    let new_start = match start {
        Some(t) => local_datetime(date, t)?,
        None => old_start,
    };
    let new_end = match end {
        Some(t) => {
            if session.is_active() {
                bail!("session is still running - run `{TOOL_NAME} session end` first.");
            }
            Some(local_datetime(date, t)?)
        }
        None => old_end,
    };
    if let Some(e) = new_end {
        if e <= new_start {
            bail!("session end must be after session start.");
        }
    }
    let new_target = target.unwrap_or(old_target);

    session.started_at = new_start;
    if start.is_some() {
        session.id = new_start.timestamp();
    }
    session.ended_at = new_end;
    session.target_secs = new_target;
    let updated = session.clone();
    store.save()?;

    if start.is_some() {
        println!(
            "Updated session start {} → {}",
            fmt_dt(old_start),
            fmt_dt(new_start)
        );
    }
    if let (Some(_), Some(e)) = (end, new_end) {
        let old = old_end.map(fmt_dt).unwrap_or_else(|| "(none)".to_string());
        println!("Updated session end {old} → {}", fmt_dt(e));
    }
    if target.is_some() {
        println!(
            "Updated session target {} → {}",
            fmt_target(old_target),
            fmt_target(new_target)
        );
    }
    print_session_summary(&updated, Local::now(), true);
    Ok(())
}

fn delete(date: NaiveDate, yes: bool) -> Result<()> {
    let mut store = Store::load()?;

    let indices = store.session_indices_on(date);
    if indices.is_empty() {
        bail!("no session found for {date}.");
    }

    let now = Local::now();
    println!("Sessions on {date} that will be deleted:");
    for &i in &indices {
        println!("  {}", fmt_session_row(&store.sessions[i], now));
    }

    if !yes {
        loop {
            print!("\nDelete {} session(s)? [y/N] ", indices.len());
            io::stdout().flush()?;

            let mut input = String::new();
            let read = io::stdin()
                .read_line(&mut input)
                .context("failed to read confirmation")?;

            // End of input or a bare Enter means "no", as the [y/N] prompt promises.
            if read == 0 {
                println!("Aborted.");
                return Ok(());
            }
            match input.trim().to_ascii_lowercase().as_str() {
                "y" | "yes" => break,
                "" | "n" | "no" => {
                    println!("Aborted.");
                    return Ok(());
                }
                _ => println!("Please answer yes or no."),
            }
        }
    }

    for &i in indices.iter().rev() {
        store.sessions.remove(i);
    }
    store.save()?;

    println!("Deleted {} session(s) on {date}.", indices.len());
    Ok(())
}
