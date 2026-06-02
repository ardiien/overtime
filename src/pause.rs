/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::store::{format_duration, Store};
use crate::util::TOOL_NAME;
use anyhow::{bail, Result};
use chrono::Local;
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
        /// Optional label for this pause (e.g. "lunch").
        #[arg(short, long)]
        label: Option<String>,
    },
    /// End the current pause.
    End,
}

pub fn handle(args: &Pause) -> Result<()> {
    match &args.command {
        Commands::Start { label } => handle_start_cmd(label.clone())?,
        Commands::End => handle_end_cmd()?,
    }

    Ok(())
}

fn handle_start_cmd(label: Option<String>) -> Result<()> {
    let mut store = Store::load()?;

    let session = match store.active_session_mut() {
        Some(s) => s,
        None => bail!(
            "no active session - run `{} record start` first.",
            TOOL_NAME
        ),
    };

    if session.has_active_pause() {
        bail!(
            "there is already a pause in progress - run `{} record pause end` first.",
            TOOL_NAME
        );
    }

    let now = Local::now();
    let display_label = label.clone().unwrap_or_else(|| "(no label)".to_string());

    session.pauses.push(crate::store::Pause {
        started_at: now,
        ended_at: None,
        label,
    });

    println!(
        "Started pause at {} [{}]",
        now.format("%Y-%m-%d %H:%M:%S"),
        display_label
    );

    store.save()?;
    Ok(())
}

fn handle_end_cmd() -> Result<()> {
    let mut store = Store::load()?;

    let session = match store.active_session_mut() {
        Some(s) => s,
        None => bail!(
            "no active session - run `{} record start` first.",
            TOOL_NAME
        ),
    };

    let pause = match session.pauses.iter_mut().find(|p| p.ended_at.is_none()) {
        Some(p) => p,
        None => bail!(
            "no active pause - run `{} record pause start` first.",
            TOOL_NAME
        ),
    };

    let now = Local::now();
    pause.ended_at = Some(now);

    let duration = pause.duration_secs().unwrap_or(0);
    let label = pause
        .label
        .clone()
        .unwrap_or_else(|| "(no label)".to_string());

    println!(
        "Ended pause at {} [{}]",
        now.format("%Y-%m-%d %H:%M:%S"),
        label,
    );
    println!("Duration {}", format_duration(duration));

    store.save()?;
    Ok(())
}
