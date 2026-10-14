/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::store::{format_duration, Store};
use crate::util;
use crate::util::TOOL_NAME;
use anyhow::{bail, Context, Result};
use chrono::{Local, NaiveTime, TimeZone};
use clap::{Args, Subcommand};
use util::{parse_target, parse_time, DEFAULT_TARGET_SECS};

#[derive(Args)]
pub struct Record {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a new work session.
    Start {
        /// Record start as a specific past time today, e.g. --at 09:00.
        #[arg(long, value_parser = parse_time)]
        at: Option<chrono::NaiveTime>,
        /// Daily target for this session as HH:MM, e.g. --target 4:00 (default 8:00).
        #[arg(long, value_parser = parse_target)]
        target: Option<i64>,
    },
    /// End the current work session.
    End {
        /// Record end as a specific past time today, e.g. --at 17:30.
        #[arg(long, value_parser = parse_time)]
        at: Option<chrono::NaiveTime>,
    },
}

pub fn handle(args: &Record) -> Result<()> {
    match &args.command {
        Commands::Start { at, target } => handle_start_cmd(at.clone(), *target)?,
        Commands::End { at } => handle_end_cmd(at.clone())?,
    }

    Ok(())
}

fn handle_start_cmd(at: Option<NaiveTime>, target: Option<i64>) -> Result<()> {
    let mut store = Store::load()?;

    if store.active_session_mut().is_some() {
        bail!(
            "there is already an active session - run `{} record end` to close it first.",
            TOOL_NAME
        );
    }

    let now = Local::now();
    let started_at = if let Some(t) = at {
        let date = now.date_naive();
        Local
            .from_local_datetime(&date.and_time(t))
            .single()
            .with_context(|| format!("ambiguous local time {t}"))?
    } else {
        now
    };

    if started_at > now {
        bail!("--at time {started_at} is in the future - did you mean yesterday?");
    }

    println!(
        "Started recording at {}",
        started_at.format("%Y-%m-%d %H:%M:%S")
    );

    let session = crate::store::Session::new(started_at, target.unwrap_or(DEFAULT_TARGET_SECS));
    store.sessions.push(session);
    store.save()?;

    Ok(())
}

fn handle_end_cmd(at: Option<NaiveTime>) -> Result<()> {
    let mut store = Store::load()?;

    let session = match store.active_session_mut() {
        Some(s) => s,
        None => bail!(
            "no active session found - run `{} record start` first.",
            TOOL_NAME
        ),
    };

    if session.has_active_pause() {
        bail!("there is an active pause in progress - end the pause before ending the session.");
    }

    let now = Local::now();
    let ended_at = if let Some(t) = at {
        let date = session.started_at.date_naive();
        Local
            .from_local_datetime(&date.and_time(t))
            .single()
            .with_context(|| format!("ambiguous local time {t}"))?
    } else {
        now
    };

    if ended_at <= session.started_at {
        bail!(
            "--at time {} is not after session start ({})",
            ended_at.format("%H:%M:%S"),
            session.started_at.format("%H:%M:%S")
        );
    }

    if ended_at > now {
        bail!(
            "--at time {} is in the future.",
            ended_at.format("%H:%M:%S")
        );
    }
    session.ended_at = Some(ended_at);

    let worked = session.worked_secs().unwrap_or(0);
    let pause_secs = session.total_pause_secs();
    let overtime = worked - session.target_secs;

    let started_at = session.started_at;
    println!(
        "Ended recording at {}",
        ended_at.format("%Y-%m-%d %H:%M:%S")
    );
    println!("Started {}", started_at.format("%Y-%m-%d %H:%M:%S"));
    println!("Pause(s) {}", format_duration(pause_secs));
    println!("Worked {}", format_duration(worked));

    if overtime >= 0 {
        println!("Overtime  +{}", format_duration(overtime));
    } else {
        println!("Undertime -{}", format_duration(-overtime));
    }

    store.save()?;
    Ok(())
}
