/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::output::{fmt_duration, fmt_label, kv, print_session_summary};
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

    if let Some(pause) = session.pauses.iter().find(|p| p.is_active()) {
        kv(
            "Pause",
            format!(
                "{} {} (running)",
                fmt_label(pause.label.as_deref()),
                fmt_duration((now - pause.started_at).num_seconds())
            ),
        );
    }

    Ok(())
}
