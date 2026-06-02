/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::util::TOOL_NAME;
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::{env, fs};

/// A single pause within a work session (e.g. lunch break).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pause {
    /// When the pause started.
    pub started_at: DateTime<Local>,
    /// When the pause ended. `None` means the pause is still ongoing.
    pub ended_at: Option<DateTime<Local>>,
    /// Optional label for the pause (e.g. "lunch").
    pub label: Option<String>,
}

impl Pause {
    /// Whether this pause is still running (no end time yet).
    pub fn is_active(&self) -> bool {
        self.ended_at.is_none()
    }

    /// Duration of the pause in seconds. Returns `None` if the pause hasn't ended.
    pub fn duration_secs(&self) -> Option<i64> {
        let end = self.ended_at?;
        Some((end - self.started_at).num_seconds())
    }
}

/// A single work session from `record start` to `record end`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique identifier (unix timestamp of start, good enough for now).
    pub id: i64,
    /// When work started.
    pub started_at: DateTime<Local>,
    /// When work ended. `None` means the session is still active.
    pub ended_at: Option<DateTime<Local>>,
    /// Zero or more pauses during this session.
    pub pauses: Vec<Pause>,
}

impl Session {
    pub fn new(started_at: DateTime<Local>) -> Self {
        Self {
            id: started_at.timestamp(),
            started_at,
            ended_at: None,
            pauses: vec![],
        }
    }

    /// Whether this session is still running (no end time yet).
    pub fn is_active(&self) -> bool {
        self.ended_at.is_none()
    }

    /// Whether there is a pause currently in progress.
    pub fn has_active_pause(&self) -> bool {
        self.pauses.iter().any(|p| p.is_active())
    }

    /// Total pause time in seconds (only counts completed pauses).
    pub fn total_pause_secs(&self) -> i64 {
        self.pauses.iter().filter_map(|p| p.duration_secs()).sum()
    }

    /// Net worked seconds: (end - start) minus all completed pauses.
    /// Returns `None` if the session hasn't ended.
    pub fn worked_secs(&self) -> Option<i64> {
        let end = self.ended_at?;
        let gross = (end - self.started_at).num_seconds();
        Some(gross - self.total_pause_secs())
    }
}

/// The entire on-disk store.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Store {
    pub sessions: Vec<Session>,
}

impl Store {
    /// Path to the data file.
    pub fn path() -> String {
        let user_config_dir = get_default_home_path();
        format!("{user_config_dir}/{}/data.json", TOOL_NAME)
    }

    /// Load from disk, creating an empty store if the file doesn't exist yet.
    pub fn load() -> Result<Self> {
        let path_str = Self::path();
        let path = Path::new(&path_str);

        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))
    }

    /// Persist to disk (creates `~/.overtime/` if needed).
    pub fn save(&self) -> Result<()> {
        let path_str = Self::path();
        let path = Path::new(&path_str);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))
    }

    /// Return the currently active (not-yet-ended) session, if any.
    pub fn active_session_mut(&mut self) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.is_active())
    }
}

/// Pretty-print a duration given as total seconds.
pub fn format_duration(total_secs: i64) -> String {
    let sign = if total_secs < 0 { "-" } else { "" };
    let total_secs = total_secs.unsigned_abs();
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{sign}{h}h {m:02}m {s:02}s")
}

#[cfg(target_os = "linux")]
#[cfg(not(test))]
fn get_default_home_path() -> String {
    env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| -> String { env::var("HOME").unwrap() + "/.config" })
}

#[cfg(target_os = "macos")]
#[cfg(not(test))]
fn get_default_home_path() -> String {
    env::var("XDG_DATA_HOME")
        .unwrap_or_else(|_| -> String { env::var("HOME").unwrap() + "/Library" })
}
