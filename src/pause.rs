/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::output::{fmt_dt, fmt_duration, fmt_label, fmt_pause_row, fmt_time, kv};
use crate::store::Store;
use crate::util::{At, TOOL_NAME, confirm, parse_at, parse_date};
use anyhow::{Result, anyhow, bail};
use chrono::{Local, NaiveDate};
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
        #[arg(short, long, value_parser = parse_at)]
        at: Option<At>,
        /// Optional label for this pause (e.g. "lunch").
        #[arg(short, long)]
        label: Option<String>,
    },
    /// End the current pause.
    End {
        /// End at a specific past time today, e.g. --at 12:30.
        #[arg(short, long, value_parser = parse_at)]
        at: Option<At>,
    },
    /// Edit a pause of a session's day.
    Edit {
        /// Date of the session (YYYY-MM-DD, today or yesterday).
        #[arg(value_parser = parse_date)]
        date: NaiveDate,
        /// 1-based pause number (see `zeit report <DATE>`).
        id: usize,
        /// New start time (HH:MM or HH:MM:SS).
        #[arg(short, long, value_parser = parse_at)]
        start: Option<At>,
        /// New end time (HH:MM or HH:MM:SS).
        #[arg(short, long, value_parser = parse_at)]
        end: Option<At>,
        /// New label.
        #[arg(short, long)]
        label: Option<String>,
    },
    /// Delete a pause of a session's day.
    Delete {
        /// Date of the session (YYYY-MM-DD, today or yesterday).
        #[arg(value_parser = parse_date)]
        date: NaiveDate,
        /// 1-based pause number (see `zeit report <DATE>`).
        id: usize,
        /// Skip the confirmation prompt.
        #[arg(short, long)]
        yes: bool,
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
        Commands::Delete { date, id, yes } => delete(*date, *id, *yes),
    }
}

/// Fail unless `id` is a valid 1-based pause number of `session`.
fn check_pause_id(session: &crate::store::Session, date: NaiveDate, id: usize) -> Result<()> {
    if id == 0 || id > session.pauses.len() {
        bail!(
            "pause {id} not found - session on {date} has {} pause(s).",
            session.pauses.len()
        );
    }
    Ok(())
}

fn start(at: Option<At>, label: Option<String>) -> Result<()> {
    let mut store = Store::load()?;

    let session = store
        .active_session_mut()
        .ok_or_else(|| anyhow!("no active session - run `{TOOL_NAME} session start` first."))?;

    if session.has_active_pause() {
        bail!("a pause is already running - run `{TOOL_NAME} pause end` first.");
    }

    let now = Local::now();
    let started_at = match at {
        Some(at) => at.resolve(now.date_naive())?,
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
    match session.pauses.iter().filter_map(|p| p.ended_at).max() {
        Some(prev_end) if started_at < prev_end => {
            bail!(
                "--at {} overlaps an earlier pause (ended {}).",
                fmt_time(started_at),
                fmt_time(prev_end)
            );
        }
        _ => {}
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

fn end(at: Option<At>) -> Result<()> {
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
        Some(at) => at.resolve(now.date_naive())?,
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
    start: Option<At>,
    end: Option<At>,
    label: Option<String>,
) -> Result<()> {
    if start.is_none() && end.is_none() && label.is_none() {
        bail!("nothing to edit - provide --start, --end or --label.");
    }

    let mut store = Store::load()?;
    let session = store
        .last_session_on_mut(date)
        .ok_or_else(|| anyhow!("no session found for {date}."))?;

    check_pause_id(session, date, id)?;
    let pause_index = id - 1;

    // Work out and validate every new value first, so a failed edit prints and saves nothing.
    let (old_start, old_end, old_label, was_active) = {
        let pause = &session.pauses[pause_index];
        (
            pause.started_at,
            pause.ended_at,
            pause.label.clone(),
            pause.is_active(),
        )
    };

    let new_start = match start {
        Some(at) => at.resolve(date)?,
        None => old_start,
    };
    let new_end = match end {
        Some(at) => {
            if was_active {
                bail!("pause is still running - run `{TOOL_NAME} pause end` first.");
            }
            Some(at.resolve(date)?)
        }
        None => old_end,
    };
    if matches!(new_end, Some(e) if e <= new_start) {
        bail!("pause end must be after pause start.");
    }
    let now = Local::now();
    let session_start = session.started_at;
    let session_end = session.ended_at.unwrap_or(now);
    let new_pause_end = new_end.unwrap_or(now);
    if new_start < session_start || new_pause_end > session_end {
        bail!("pause times must stay within the session.");
    }
    if new_start > now || new_pause_end > now {
        bail!("pause times cannot be in the future.");
    }
    for (index, other) in session.pauses.iter().enumerate() {
        if index == pause_index {
            continue;
        }
        let other_end = other.ended_at.unwrap_or(now);
        if new_start < other_end && other.started_at < new_pause_end {
            bail!("pause times cannot overlap another pause.");
        }
    }

    {
        let pause = &mut session.pauses[pause_index];
        pause.started_at = new_start;
        pause.ended_at = new_end;
        if let Some(l) = &label {
            pause.label = Some(l.clone());
        }
    }
    let duration = session.pauses[pause_index].duration_secs();
    store.save()?;

    if start.is_some() {
        println!(
            "Updated pause {id} start {} → {}",
            crate::output::paint(fmt_dt(old_start), crate::output::Style::Dim),
            crate::output::paint(fmt_dt(new_start), crate::output::Style::Bold)
        );
    }
    if let (Some(_), Some(e)) = (end, new_end) {
        let old = old_end.map(fmt_dt).unwrap_or_else(|| "(none)".to_string());
        println!(
            "Updated pause {id} end {} → {}",
            crate::output::paint(old, crate::output::Style::Dim),
            crate::output::paint(fmt_dt(e), crate::output::Style::Bold)
        );
    }
    if let Some(l) = &label {
        println!(
            "Updated pause {id} label {} → {}",
            crate::output::paint(fmt_label(old_label.as_deref()), crate::output::Style::Dim),
            crate::output::paint(fmt_label(Some(l)), crate::output::Style::Bold)
        );
    }
    if let Some(d) = duration {
        kv("Duration", fmt_duration(d));
    }
    Ok(())
}

fn delete(date: NaiveDate, id: usize, yes: bool) -> Result<()> {
    let mut store = Store::load()?;
    let session = store
        .last_session_on_mut(date)
        .ok_or_else(|| anyhow!("no session found for {date}."))?;
    check_pause_id(session, date, id)?;

    println!("Pause on {date} that will be deleted:");
    println!(
        "  {}",
        fmt_pause_row(id, &session.pauses[id - 1], Local::now())
    );

    if !yes && !confirm("Delete this pause?")? {
        println!(
            "{}",
            crate::output::paint("Aborted.", crate::output::Style::Warning)
        );
        return Ok(());
    }

    session.pauses.remove(id - 1);
    let renumbered = id <= session.pauses.len();
    store.save()?;

    println!("Deleted pause {id} on {date}.");
    if renumbered {
        println!("Later pauses moved up by one - check `{TOOL_NAME} report {date}`.");
    }
    Ok(())
}
