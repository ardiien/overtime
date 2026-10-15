/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::store::{Store, format_duration, format_target};
use anyhow::Result;
use chrono::Local;
use clap::Args;

#[derive(Args)]
pub struct Status {}

pub fn handle(_args: &Status) -> Result<()> {
    let store = Store::load()?;

    let session = match store.sessions.iter().rev().find(|s| s.is_active()) {
        Some(s) => s,
        None => {
            println!("No active session.");
            return Ok(());
        }
    };

    let now = Local::now();
    let elapsed = (now - session.started_at).num_seconds();
    let pause_secs = session.total_pause_secs();
    let worked = elapsed - pause_secs;

    println!(
        "Started at {} (Target: {})",
        session.started_at.format("%Y-%m-%d %H:%M:%S"),
        format_target(session.target_secs),
    );
    println!("Elapsed {}", format_duration(elapsed));
    println!("Pause(s) {}", format_duration(pause_secs));
    println!("Worked {}", format_duration(worked));

    if session.has_active_pause() {
        let pause = session
            .pauses
            .iter()
            .rev()
            .find(|p| p.ended_at.is_none())
            .unwrap();
        let pause_elapsed = (now - pause.started_at).num_seconds();
        let label = pause.label.as_deref().unwrap_or("(no label)");
        println!(
            "\nActive pause: {} - {}",
            label,
            format_duration(pause_elapsed)
        );
    }

    Ok(())
}
