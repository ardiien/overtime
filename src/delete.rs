/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::store::{format_duration, Store};
use crate::util::parse_date;
use anyhow::{bail, Context};
use chrono::NaiveDate;
use clap::Args;
use std::io;
use std::io::Write;

#[derive(Args)]
pub struct Delete {
    /// Date to delete (YYYY-MM-DD).
    #[arg(value_parser = parse_date)]
    date: NaiveDate,
    /// Skip the confirmation prompt.
    #[arg(short, long, default_value_t = false)]
    yes: bool,
}

pub fn handle(args: &Delete) -> anyhow::Result<()> {
    let mut store = Store::load()?;

    let indices: Vec<usize> = store
        .sessions
        .iter()
        .enumerate()
        .filter(|(_, s)| s.started_at.date_naive() == args.date)
        .map(|(i, _)| i)
        .collect();

    if indices.is_empty() {
        bail!("no sessions found for {}.", args.date);
    }

    println!("Record(s) on {} that will be deleted:", args.date);
    let mut j = 0;
    for &i in &indices {
        let s = &store.sessions[i];
        let end_str = s
            .ended_at
            .map(|e| e.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "ongoing".to_string());
        let worked_str = s
            .worked_secs()
            .map(format_duration)
            .unwrap_or_else(|| "—".to_string());
        println!(
            "[{}] {} → {} worked {}, {} pause(s)",
            j + 1,
            s.started_at.format("%H:%M:%S"),
            end_str,
            worked_str,
            s.pauses.len(),
        );
        j = j + 1;
    }

    if !args.yes {
        loop {
            print!("\nDelete {} record(s)? [y/N] ", indices.len());
            io::stdout().flush()?;

            let mut input = String::new();

            io::stdin()
                .read_line(&mut input)
                .context("failed to read confirmation")?;

            match input.trim().to_ascii_lowercase().as_str() {
                "y" | "yes" => break,

                "n" | "no" => {
                    println!("Aborted.");
                    return Ok(());
                }

                _ => {
                    println!("Please answer yes or no.");
                }
            }
        }
    }

    for &i in indices.iter().rev() {
        store.sessions.remove(i);
    }

    store.save()?;
    println!("Deleted {} record(s) for {}.", indices.len(), args.date);

    Ok(())
}
