/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

/// Track your overtime hours from the command line.
#[derive(Parser)]
#[command(name = "zeit", version, about, long_about = None, arg_required_else_help(true))]
struct Overtime {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Record work sessions.
    Record(crate::record::Record),
    /// Show the current active session and pauses.
    Status(crate::status::Status),
    /// Manage pauses within a session.
    Pause(crate::pause::Pause),
    /// Show a report.
    /// No flags result in all-time summary, broken down by year.
    Report(crate::report::Report),
    /// Edit a session's start/end date-time or a pause within it.
    Edit(crate::edit::Edit),
    /// Delete all sessions for a given date.
    Delete(crate::delete::Delete),
}

pub fn execute() -> Result<()> {
    let overtime = Overtime::parse();

    match &overtime.command {
        Commands::Record(args) => {
            crate::record::handle(args).context("'record' command exception")?
        }
        Commands::Status(args) => {
            crate::status::handle(args).context("'status' command exception")?
        }
        Commands::Pause(args) => crate::pause::handle(args).context("'pause' command exception")?,
        Commands::Report(args) => {
            crate::report::handle(args).context("'report' command exception")?
        }
        Commands::Edit(args) => crate::edit::handle(args).context("'edit' command exception")?,
        Commands::Delete(args) => {
            crate::delete::handle(args).context("'delete' command exception")?
        }
    }

    Ok(())
}
