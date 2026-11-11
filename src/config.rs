/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::output::{fmt_target, kv};
use crate::store::Store;
use crate::util::parse_target;
use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct Config {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show or change the default daily target.
    Target {
        /// New default target in HH:MM format.
        value: Option<String>,
    },
    /// Restore the default daily target to 08:00.
    ResetTarget,
}

pub fn handle(args: &Config) -> Result<()> {
    match &args.command {
        Commands::Target { value: None } => {
            kv("Target", fmt_target(Store::default_target_secs()?));
            Ok(())
        }
        Commands::Target { value: Some(value) } => {
            let target = parse_target(value)?;
            Store::set_default_target_secs(target)?;
            println!("Default target set to {}.", fmt_target(target));
            Ok(())
        }
        Commands::ResetTarget => {
            Store::set_default_target_secs(crate::util::DEFAULT_TARGET_SECS)?;
            println!("Default target reset to 08:00.");
            Ok(())
        }
    }
}
