/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::output::{Style, fmt_duration, fmt_label, fmt_time, kv, paint, print_session_summary};
use crate::store::Store;
use anyhow::Result;
use chrono::Local;
use clap::Args;

#[derive(Args)]
pub struct Status {}

pub fn handle(_args: &Status) -> Result<()> {
    let store = Store::load()?;

    let session = match store.sessions.iter().find(|s| s.is_active()) {
        Some(s) => s,
        None => {
            println!("No active session.");
            return Ok(());
        }
    };

    let now = Local::now();
    print_session_summary(session, now, true);
    let worked = session.worked_secs_at(now);
    let target = session.target_secs;
    let remaining = (target - worked).max(0);
    let percent = if target <= 0 {
        100
    } else {
        ((worked.max(0) as i128 * 100 / target as i128).min(100)) as u32
    };
    let filled = percent as usize / 5;
    let bar = format!(
        "[{}{}] {percent}%",
        "#".repeat(filled),
        "-".repeat(20 - filled)
    );
    kv(
        "Progress",
        paint(
            bar,
            if percent == 100 {
                Style::Good
            } else {
                Style::Bold
            },
        ),
    );
    if worked >= target {
        if let Some(reached) = target_reached_at(session, now) {
            kv("Target at", fmt_time(reached));
        }
    } else if session.has_active_pause() {
        kv(
            "Target in",
            format!("{} work time", fmt_duration(remaining)),
        );
    } else if let Some(eta) = now.checked_add_signed(Duration::seconds(remaining)) {
        kv("Target ETA", fmt_time(eta));
    }

    if let Some(pause) = session.pauses.iter().find(|p| p.is_active()) {
        kv(
            "Pause",
            format!(
                "{} {} {}",
                fmt_label(pause.label.as_deref()),
                fmt_duration((now - pause.started_at).num_seconds()),
                paint("(running)", Style::Warning)
            ),
        );
    }

    Ok(())
}
