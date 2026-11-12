/*
 * Copyright 2026 ardiien
 * Licensed under the Apache License, Version 2.0.
 * See http://www.apache.org/licenses/LICENSE-2.0
 */

use crate::output::{
    Style, fmt_duration, fmt_label, fmt_signed, fmt_target, fmt_time, kv_indented_no_colon, paint,
};
use crate::store::{Session, Store};
use anyhow::Result;
use chrono::{DateTime, Duration, Local};
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
    print_section(format!("Session {}", session.started_at.date_naive()));
    kv_indented_no_colon("Started", fmt_time(session.started_at));
    kv_indented_no_colon(
        "Elapsed",
        fmt_duration((now - session.started_at).num_seconds()),
    );

    let worked = session.worked_secs_at(now);
    let target = session.target_secs;
    println!();
    print_section("Work");
    kv_indented_no_colon("Worked", fmt_duration(worked));
    kv_indented_no_colon("Pauses", fmt_duration(session.pause_secs_at(now)));
    if let Some(pause) = session.pauses.iter().find(|p| p.is_active()) {
        kv_indented_no_colon("Pause", fmt_active_pause(pause, now));
    }

    let percent = if target <= 0 {
        100
    } else {
        ((worked.max(0) as i128 * 100 / target as i128).min(100)) as u32
    };
    let filled = percent as usize / 5;
    let bar = format!(
        "[{}{}] {percent}%",
        "#".repeat(filled),
        "-".repeat(20 - filled)
    );
    if worked >= target {
        if let Some(reached) = target_reached_at(session, now) {
            kv_indented_no_colon("Target reached", fmt_time(reached));
        }
    } else if let Some(eta) = target_eta(session, now) {
        kv_indented_no_colon("End ETA", fmt_time(eta));
    }

    println!();
    print_section(format!("Target {}", fmt_target(target)));
    kv_indented_no_colon(
        "Progress",
        paint(
            bar,
            if percent == 100 {
                Style::Good
            } else {
                Style::Bold
            },
        ),
    );
    kv_indented_no_colon("Overtime", fmt_signed(worked - target));

    Ok(())
}

fn print_section(title: impl std::fmt::Display) {
    println!("{}", paint(title, Style::Bold));
}

fn target_eta(session: &Session, now: DateTime<Local>) -> Option<DateTime<Local>> {
    let remaining = (session.target_secs - session.worked_secs_at(now)).max(0);
    now.checked_add_signed(Duration::seconds(remaining))
}

fn fmt_active_pause(pause: &crate::store::Pause, now: DateTime<Local>) -> String {
    format!(
        "{} {} {}",
        fmt_duration((now - pause.started_at).num_seconds()),
        fmt_label(pause.label.as_deref()),
        paint("(ongoing)", Style::Warning)
    )
}

fn target_reached_at(
    session: &Session,
    now: chrono::DateTime<Local>,
) -> Option<chrono::DateTime<Local>> {
    let mut remaining = session.target_secs;
    let mut cursor = session.started_at;
    let mut pauses: Vec<_> = session.pauses.iter().collect();
    pauses.sort_by_key(|pause| pause.started_at);

    for pause in pauses {
        let worked_before_pause = (pause.started_at - cursor).num_seconds();
        if remaining <= worked_before_pause {
            return cursor.checked_add_signed(Duration::seconds(remaining));
        }
        remaining -= worked_before_pause;
        cursor = pause.ended_at?;
    }

    if remaining <= (now - cursor).num_seconds() {
        cursor.checked_add_signed(Duration::seconds(remaining))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Pause;
    use chrono::{NaiveDate, NaiveTime, TimeZone};

    fn dt(hour: u32) -> DateTime<Local> {
        Local
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2026, 5, 27)
                    .unwrap()
                    .and_time(NaiveTime::from_hms_opt(hour, 0, 0).unwrap()),
            )
            .single()
            .unwrap()
    }

    #[test]
    fn target_eta_accounts_for_active_pause() {
        let session = Session {
            id: 1,
            started_at: dt(9),
            ended_at: None,
            pauses: vec![Pause {
                started_at: dt(12),
                ended_at: None,
                label: None,
            }],
            target_secs: 8 * 60 * 60,
        };

        assert_eq!(target_eta(&session, dt(13)), Some(dt(18)));
    }

    #[test]
    fn active_pause_status_shows_time_before_label() {
        crate::output::set_color_enabled(false);
        let pause = Pause {
            started_at: dt(12),
            ended_at: None,
            label: Some("lunch".to_string()),
        };

        assert_eq!(
            fmt_active_pause(&pause, dt(13)),
            "01:00:00 [lunch] (ongoing)"
        );
    }
}
