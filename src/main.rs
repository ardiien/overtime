/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

mod output;
mod overtime;
mod pause;
mod report;
mod session;
mod status;
mod store;
mod util;

use std::process::ExitCode;

fn main() -> ExitCode {
    match overtime::execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
