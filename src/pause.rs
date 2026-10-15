/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::output::{fmt_dt, fmt_duration, fmt_label, fmt_time, kv};
use crate::store::Store;
use crate::util::{local_datetime, parse_date, parse_time, TOOL_NAME};
use anyhow::{anyhow, bail, Result};
use chrono::{Local, NaiveDate, NaiveTime};
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct Pause {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a pause (e.g. lunch break).
    Start {
        /// Start at a specific past time today, e.g. --at 12:00.
        #[arg(short, long, value_parser = parse_time)]
        at: Option<NaiveTime>,
        /// Optional label for this pause (e.g. "lunch").
        #[arg(short, long)]
        label: Option<String>,
    },
    /// End the current pause.
    End {
        /// End at a specific past time today, e.g. --at 12:30.
        #[arg(short, long, value_parser = parse_time)]
        at: Option<NaiveTime>,
    },
    /// Edit a pause of a session's day.
    Edit {
        /// Date of the session the pause belongs to (YYYY-MM-DD).
        #[arg(value_parser = parse_date)]
        date: NaiveDate,
        /// 1-based pause number (see `zeit report <DATE>`).
        id: usize,
        /// New start time (HH:MM or HH:MM:SS).
        #[arg(short, long, value_parser = parse_time)]
        start: Option<NaiveTime>,
        /// New end time (HH:MM or HH:MM:SS).
        #[arg(short, long, value_parser = parse_time)]
        end: Option<NaiveTime>,
        /// New label.
        #[arg(short, long)]
        label: Option<String>,
    },
}

pub fn handle(args: &Pause) -> Result<()> {
    match &args.command {
        Commands::Start { at, label } => start(*at, label.clone()),
        Commands::End { at } => end(*at),
        Commands::Edit {
            date,
            id,
            start,
            end,
            label,
        } => edit(*date, *id, *start, *end, label.clone()),
    }
}

fn start(at: Option<NaiveTime>, label: Option<String>) -> Result<()> {
    let mut store = Store::load()?;

    let session = store
        .active_session_mut()
        .ok_or_else(|| anyhow!("no active session - run `{TOOL_NAME} session start` first."))?;

    if session.has_active_pause() {
        bail!("a pause is already running - run `{TOOL_NAME} pause end` first.");
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
    if started_at < session.started_at {
        bail!(
            "--at {} is before session start ({}).",
            fmt_time(started_at),
            fmt_time(session.started_at)
        );
    }
    if let Some(prev_end) = session.pauses.iter().filter_map(|p| p.ended_at).max() {
        if started_at < prev_end {
            bail!(
                "--at {} overlaps an earlier pause (ended {}).",
                fmt_time(started_at),
                fmt_time(prev_end)
            );
        }
    }

    session.pauses.push(crate::store::Pause {
        started_at,
        ended_at: None,
        label: label.clone(),
    });
    store.save()?;

    println!(
        "Started pause at {} {}",
        fmt_dt(started_at),
        fmt_label(label.as_deref())
    );
    Ok(())
}

fn end(at: Option<NaiveTime>) -> Result<()> {
    let mut store = Store::load()?;

    let session = store
        .active_session_mut()
        .ok_or_else(|| anyhow!("no active session - run `{TOOL_NAME} session start` first."))?;

    let pause = session
        .pauses
        .iter_mut()
        .find(|p| p.is_active())
        .ok_or_else(|| anyhow!("no active pause - run `{TOOL_NAME} pause start` first."))?;

    let now = Local::now();
    let ended_at = match at {
        Some(t) => local_datetime(now.date_naive(), t)?,
        None => now,
    };
    if ended_at <= pause.started_at {
        bail!(
            "--at {} is not after pause start ({}).",
            fmt_time(ended_at),
            fmt_time(pause.started_at)
        );
    }
    if ended_at > now {
        bail!(
            "--at {} is in the future - use a time up to now.",
            fmt_time(ended_at)
        );
    }

    pause.ended_at = Some(ended_at);
    let duration = pause.duration_secs().unwrap_or(0);
    let label = pause.label.clone();
    store.save()?;

    println!(
        "Ended pause at {} {}",
        fmt_dt(ended_at),
        fmt_label(label.as_deref())
    );
    kv("Duration", fmt_duration(duration));
    Ok(())
}

fn edit(
    date: NaiveDate,
    id: usize,
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
    label: Option<String>,
) -> Result<()> {
    if start.is_none() && end.is_none() && label.is_none() {
        bail!("nothing to edit - provide --start, --end or --label.");
    }

    let mut store = Store::load()?;
    let idx = *store
        .session_indices_on(date)
        .last()
        .ok_or_else(|| anyhow!("no session found for {date}."))?;
    let session = &mut store.sessions[idx];

    if id == 0 || id > session.pauses.len() {
        bail!(
            "pause {id} not found - session on {date} has {} pause(s).",
            session.pauses.len()
        );
    }
    let pause = &mut session.pauses[id - 1];

    // Work out and validate every new value first, so a failed edit prints and saves nothing.
    let old_start = pause.started_at;
    let old_end = pause.ended_at;
    let old_label = pause.label.clone();

    let new_start = match start {
        Some(t) => local_datetime(date, t)?,
        None => old_start,
    };
    let new_end = match end {
        Some(t) => {
            if pause.is_active() {
                bail!("pause is still running - run `{TOOL_NAME} pause end` first.");
            }
            Some(local_datetime(date, t)?)
        }
        None => old_end,
    };
    if let Some(e) = new_end {
        if e <= new_start {
            bail!("pause end must be after pause start.");
        }
    }

    pause.started_at = new_start;
    pause.ended_at = new_end;
    if let Some(l) = &label {
        pause.label = Some(l.clone());
    }
    let duration = pause.duration_secs();
    store.save()?;

    if start.is_some() {
        println!(
            "Updated pause {id} start {} → {}",
            fmt_dt(old_start),
            fmt_dt(new_start)
        );
    }
    if let (Some(_), Some(e)) = (end, new_end) {
        let old = old_end.map(fmt_dt).unwrap_or_else(|| "(none)".to_string());
        println!("Updated pause {id} end {old} → {}", fmt_dt(e));
    }
    if let Some(l) = &label {
        println!(
            "Updated pause {id} label {} → {}",
            fmt_label(old_label.as_deref()),
            fmt_label(Some(l))
        );
    }
    if let Some(d) = duration {
        kv("Duration", fmt_duration(d));
    }
    Ok(())
}
