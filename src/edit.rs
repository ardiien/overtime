/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::store::{format_duration, Store};
use crate::util::{format_hhmm, parse_date, parse_target, parse_time};
use anyhow::{bail, Context};
use chrono::{Local, NaiveDate, NaiveTime, TimeZone};
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct Edit {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Edit a session.
    Record {
        /// Date of the session to edit (YYYY-MM-DD).
        #[arg(value_parser = parse_date)]
        date: NaiveDate,
        /// New start time for the session (HH:MM or HH:MM:SS).
        #[arg(short, long, value_parser = parse_time)]
        start: Option<NaiveTime>,
        /// New end time for the session (HH:MM or HH:MM:SS).
        #[arg(short, long, value_parser = parse_time)]
        end: Option<NaiveTime>,
        /// New daily target for the session as HH:MM, e.g. --target 4:00.
        #[arg(short, long, value_parser = parse_target)]
        target: Option<i64>,
    },
    /// Edit a session's pause.
    Pause {
        /// Date of the session to edit (YYYY-MM-DD).
        #[arg(value_parser = parse_date)]
        date: NaiveDate,
        /// 1-based pause index to edit (see `report --day N`).
        #[arg()]
        id: usize,
        /// New start time for the selected pause.
        #[arg(short, long, value_parser = parse_time)]
        start: Option<NaiveTime>,
        /// New end time for the selected pause.
        #[arg(short, long, value_parser = parse_time)]
        end: Option<NaiveTime>,
        /// New label for the selected pause.
        #[arg(short, long)]
        label: Option<String>,
    },
}

pub fn handle(args: &Edit) -> anyhow::Result<()> {
    match &args.command {
        Commands::Record {
            date,
            start,
            end,
            target,
        } => handle_record_cmd(date.clone(), start.clone(), end.clone(), *target)?,
        Commands::Pause {
            date,
            id,
            start,
            end,
            label,
        } => handle_pause_cmd(
            date.clone(),
            id.clone(),
            start.clone(),
            end.clone(),
            label.clone(),
        )?,
    }

    Ok(())
}

fn handle_record_cmd(
    date: NaiveDate,
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
    target: Option<i64>,
) -> anyhow::Result<()> {
    let mut store = Store::load()?;

    if store.sessions.is_empty() {
        bail!("no sessions found.");
    }

    let session_idx = store
        .sessions
        .iter()
        .rposition(|s| s.started_at.date_naive() == date)
        .with_context(|| format!("no sessions found for {date}"))?;

    let session = &mut store.sessions[session_idx];
    let date = session.started_at.date_naive();

    if start.is_none() && end.is_none() && target.is_none() {
        bail!("nothing to edit - provide --start, --end, or --target.");
    }

    let old_start = session.started_at;
    let old_end = session.ended_at;

    if let Some(t) = start {
        let new_dt = Local
            .from_local_datetime(&date.and_time(t))
            .single()
            .with_context(|| format!("ambiguous local time {t}"))?;
        session.started_at = new_dt;
        session.id = new_dt.timestamp();

        println!(
            "Updated record start {} → {}",
            old_start.format("%Y-%m-%d %H:%M:%S"),
            new_dt.format("%Y-%m-%d %H:%M:%S")
        );
    }

    if let Some(t) = end {
        if session.is_active() {
            bail!("session is still active - end it with `record end` first.");
        }

        let new_dt = Local
            .from_local_datetime(&date.and_time(t))
            .single()
            .with_context(|| format!("ambiguous local time {t}"))?;
        let old_end_str = old_end
            .map(|e| e.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "(none)".to_string());
        session.ended_at = Some(new_dt);

        println!(
            "Updated record end {old_end_str} → {}",
            new_dt.format("%Y-%m-%d %H:%M:%S")
        );
    }

    if let Some(new_target) = target {
        let old_target = session.target_secs;
        session.target_secs = new_target;
        println!(
            "Updated daily target {} → {}",
            format_hhmm(old_target),
            format_hhmm(new_target)
        );
    }

    let session = &store.sessions[session_idx];
    if let Some(end) = session.ended_at {
        if end <= session.started_at {
            bail!("session end must be after session start.");
        }

        if let Some(worked) = session.worked_secs() {
            println!("Worked {}", format_duration(worked));
        }
    }

    store.save()?;
    Ok(())
}

fn handle_pause_cmd(
    date: NaiveDate,
    id: usize,
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
    label: Option<String>,
) -> anyhow::Result<()> {
    let mut store = Store::load()?;

    if store.sessions.is_empty() {
        bail!("no sessions found.");
    }

    let session_idx = store
        .sessions
        .iter()
        .rposition(|s| s.started_at.date_naive() == date)
        .with_context(|| format!("no session found for {date}"))?;

    let session = &mut store.sessions[session_idx];
    let date = session.started_at.date_naive();

    if id == 0 || id > session.pauses.len() {
        bail!(
            "pause index {id} is out of range - session on {date} has {} pause(s).",
            session.pauses.len()
        );
    }

    if start.is_none() && end.is_none() && label.is_none() {
        bail!("nothing to edit - provide --start, --end, or --label.");
    }

    let pause = &mut session.pauses[id - 1];

    if let Some(t) = start {
        let new_dt = Local
            .from_local_datetime(&date.and_time(t))
            .single()
            .with_context(|| format!("ambiguous local time for pause start {t}"))?;
        let old = pause.started_at;
        pause.started_at = new_dt;

        println!(
            "Updated pause {id} start {} → {}",
            old.format("%H:%M:%S"),
            new_dt.format("%H:%M:%S")
        );
    }

    if let Some(t) = end {
        if pause.is_active() {
            bail!("pause is still active - end it with `pause end` first.");
        }

        let new_dt = Local
            .from_local_datetime(&date.and_time(t))
            .single()
            .with_context(|| format!("ambiguous local time for pause end {t}"))?;
        let old_str = pause
            .ended_at
            .map(|e| e.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "(none)".to_string());
        pause.ended_at = Some(new_dt);

        println!(
            "Updated pause {id} end {old_str} → {}",
            new_dt.format("%H:%M:%S")
        );
    }

    if let Some(label) = label {
        let old = pause.label.clone().unwrap_or_else(|| "(none)".to_string());
        pause.label = Some(label.clone());
        println!("Updated pause {id} label {old:?} → {label:?}");
    }

    let pause = &session.pauses[id - 1];
    if let Some(end) = pause.ended_at {
        if end <= pause.started_at {
            bail!("pause end must be after pause start.");
        }
    }

    store.save()?;
    Ok(())
}
