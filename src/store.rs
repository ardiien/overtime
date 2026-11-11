/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::util::{DEFAULT_TARGET_SECS, TOOL_NAME};
use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{env, fs, io::Write};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

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

/// A single work session from `session start` to `session end`.
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
    /// Daily target in seconds. Overtime = worked - target.
    /// Sessions saved before this field existed deserialize as the 8h default.
    #[serde(default = "default_target_secs")]
    pub target_secs: i64,
}

fn default_target_secs() -> i64 {
    DEFAULT_TARGET_SECS
}

impl Session {
    pub fn new(started_at: DateTime<Local>, target_secs: i64) -> Self {
        Self {
            id: started_at.timestamp(),
            started_at,
            ended_at: None,
            pauses: vec![],
            target_secs,
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

    /// Total pause time in seconds as of `at`. A pause still running counts up to `at`.
    pub fn pause_secs_at(&self, at: DateTime<Local>) -> i64 {
        self.pauses
            .iter()
            .map(|p| (p.ended_at.unwrap_or(at) - p.started_at).num_seconds())
            .sum()
    }

    /// Net worked seconds as of `at`: (at - start) minus all pause time.
    pub fn worked_secs_at(&self, at: DateTime<Local>) -> i64 {
        (at - self.started_at).num_seconds() - self.pause_secs_at(at)
    }

    /// Net worked seconds of a finished session. `None` if the session hasn't ended.
    pub fn worked_secs(&self) -> Option<i64> {
        self.ended_at.map(|end| self.worked_secs_at(end))
    }
}

/// The entire on-disk store.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Store {
    pub sessions: Vec<Session>,
}

impl Store {
    /// Path to the data file.
    pub fn path() -> Result<PathBuf> {
        if let Some(path) = env::var_os("ZEIT_DATA").filter(|path| !path.is_empty()) {
            return Ok(PathBuf::from(path));
        }
        Ok(get_default_home_path()?.join(TOOL_NAME).join("data.json"))
    }

    pub fn config_path() -> Result<PathBuf> {
        let data_path = Self::path()?;
        Ok(data_path
            .parent()
            .context("data path has no parent directory")?
            .join("config.json"))
    }

    pub fn default_target_secs() -> Result<i64> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(DEFAULT_TARGET_SECS);
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let config: Preferences = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let target = config.default_target_secs.unwrap_or(DEFAULT_TARGET_SECS);
        anyhow::ensure!(
            target >= 0,
            "default target in {} cannot be negative",
            path.display()
        );
        Ok(target)
    }

    pub fn set_default_target_secs(target_secs: i64) -> Result<()> {
        let path = Self::config_path()?;
        let contents = serde_json::to_vec_pretty(&Preferences {
            default_target_secs: Some(target_secs),
        })?;
        atomic_write(&path, &contents)
    }

    /// Load from disk, creating an empty store if the file doesn't exist yet.
    pub fn load() -> Result<Self> {
        let path = Self::path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))
    }

    /// Persist to disk (creates the data directory if needed).
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let contents = serde_json::to_vec_pretty(self)?;
        atomic_write(&path, &contents)
    }

    /// Return the currently active (not-yet-ended) session, if any.
    pub fn active_session_mut(&mut self) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.is_active())
    }

    /// Return the last session started on `date`, if any.
    pub fn last_session_on_mut(&mut self, date: NaiveDate) -> Option<&mut Session> {
        let index = self.session_indices_on(date).last().copied()?;
        self.sessions.get_mut(index)
    }

    /// Indices of all sessions that started on `date`, oldest first.
    pub fn session_indices_on(&self, date: NaiveDate) -> Vec<usize> {
        self.sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| s.started_at.date_naive() == date)
            .map(|(i, _)| i)
            .collect()
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Preferences {
    #[serde(default)]
    default_target_secs: Option<i64>,
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .context("data file path has no filename")?
        .to_string_lossy();
    let temporary_path = path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        counter
    ));

    let write_result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .with_context(|| format!("failed to create {}", temporary_path.display()))?;
        file.write_all(contents)
            .with_context(|| format!("failed to write {}", temporary_path.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync {}", temporary_path.display()))?;
        fs::rename(&temporary_path, path)
            .with_context(|| format!("failed to replace {}", path.display()))?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
}

#[cfg(target_os = "linux")]
fn get_default_home_path() -> Result<PathBuf> {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(path));
    }
    let home = env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".config"))
}

#[cfg(target_os = "macos")]
fn get_default_home_path() -> Result<PathBuf> {
    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(path));
    }
    let home = env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join("Library"))
}
