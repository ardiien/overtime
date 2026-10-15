/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use anyhow::Result;
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
    /// Start, end, edit or delete work sessions.
    Session(crate::session::Session),
    /// Start, end, edit or delete pauses within a session.
    Pause(crate::pause::Pause),
    /// Show the current active session and pause.
    Status(crate::status::Status),
    /// Show a report for a year, month, day or all time.
    Report(crate::report::Report),
}

pub fn execute() -> Result<()> {
    let overtime = Overtime::parse();

    match &overtime.command {
        Commands::Session(args) => crate::session::handle(args),
        Commands::Pause(args) => crate::pause::handle(args),
        Commands::Status(args) => crate::status::handle(args),
        Commands::Report(args) => crate::report::handle(args),
    }
}
