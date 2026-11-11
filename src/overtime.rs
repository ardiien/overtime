/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use std::io;
use crate::util::TOOL_NAME;

/// Track your overtime hours from the command line.
#[derive(Parser)]
#[command(name = "zeit", version, about, long_about = None, arg_required_else_help(true))]
struct Overtime {
    /// Control colored output.
    #[arg(long, global = true, value_enum, default_value_t = ColorOption::Auto)]
    color: ColorOption,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum ColorOption {
    #[default]
    Auto,
    Always,
    Never,
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
    /// Generate shell completion scripts.
    Completions {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// View or change local preferences.
    Config(crate::config::Config),
}

pub fn execute() -> Result<()> {
    let overtime = Overtime::parse();
    let color = match overtime.color {
        ColorOption::Auto => {
            anstream::AutoStream::choice(&io::stdout()) != anstream::ColorChoice::Never
        }
        ColorOption::Always => true,
        ColorOption::Never => false,
    };
    crate::output::set_color_enabled(color);

    match &overtime.command {
        Commands::Session(args) => crate::session::handle(args),
        Commands::Pause(args) => crate::pause::handle(args),
        Commands::Status(args) => crate::status::handle(args),
        Commands::Report(args) => crate::report::handle(args),
        Commands::Completions { shell } => {
            let mut command = Overtime::command();
            clap_complete::generate(*shell, &mut command, TOOL_NAME, &mut io::stdout());
            Ok(())
        }
        Commands::Config(args) => crate::config::handle(args),
    }
}
