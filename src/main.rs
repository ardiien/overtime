/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

mod delete;
mod edit;
mod overtime;
mod pause;
mod record;
mod report;
mod status;
mod store;
mod util;

fn main() -> anyhow::Result<()> {
    overtime::execute()
}
