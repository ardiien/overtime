# zeit

A minimal Rust CLI for tracking work sessions, pauses, and overtime. 

`zeit` stores your sessions locally as JSON and provides commands for recording work time, managing breaks, editing entries, and generating overtime reports.

---

## Installation

### From source

Requirements:

* Rust 1.85+
* Cargo

Clone and install:

```bash
cargo install --path . # add --force to reinstall
```

If Cargo's bin directory is not in your `PATH`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Verify installation:

```bash
zeit --help
```

---

## Usage

```bash
zeit <COMMAND>
```

| Command          | Description                                   |
|------------------|-----------------------------------------------|
| `session`        | Start, end, edit or delete work sessions      |
| `pause`          | Start, end or edit pauses within a session    |
| `status`         | Show the active session and pause             |
| `report`         | Overtime report for a year, month, day or all |

### Conventions

Every command follows the same rules:

* **Shape:** `zeit <noun> <verb>`: `session start|end|edit|delete`, `pause start|end|edit`.
  `status` and `report` only read data.
* **Flags:** `--at` says *when* an action happens (`start`/`end`). `--start`/`--end` are the
  *new values* when editing. Each short flag means the same thing everywhere:
  `-a` at, `-s` start, `-e` end, `-l` label, `-t` target, `-y` yes.
* **Input formats:** dates `YYYY-MM-DD`, times `HH:MM` or `HH:MM:SS`, daily targets `HH:MM`.
  Report periods are `YYYY`, `YYYY-MM`, `YYYY-MM-DD` or `today`.
* **Output:** durations are `HH:MM:SS` (targets are `HH:MM`), overtime is always signed
  (`+00:12:09`, `-01:30:00`; negative means undertime), pause labels are `[label]`.
* **Errors:** printed as `error: <problem> - <what to do>.`

---

# Sessions

## Start a session

```bash
zeit session start
```

Start at a specific past time today:

```bash
zeit session start --at 09:00
```

Start with a different daily target (`HH:MM`, default `08:00`):

```bash
zeit session start --target 4:00
zeit session start --at 09:00 --target 6:30
```

Start a session with a different daily target (`HH:MM`, default `8:00`):

```bash
zeit record start --target 4:00
zeit record start --at 09:00 --target 6:30
```

## End a session

```bash
zeit session end
zeit session end --at 17:30
```

Example:

```text
Ended session at 2026-05-27 17:42:10
Started:  2026-05-27 08:58:01
Pauses:   00:32:00
Worked:   08:12:09
Target:   08:00
Overtime: +00:12:09
```

## Edit a session

```bash
zeit session edit 2026-05-27 --start 09:00
zeit session edit 2026-05-27 --end 17:45
zeit session edit 2026-05-27 --target 4:00
zeit session edit 2026-05-27 -s 09:00 -e 13:00 -t 4:00
```

Nothing is changed or saved if any of the new values is invalid. If several
sessions started on the same date, the last one is edited.

## Delete sessions

Delete all sessions on a date (asks for confirmation, Enter means no):

```bash
zeit session delete 2026-05-27
zeit session delete 2026-05-27 --yes
```

---

# Pauses

## Start a pause

```bash
zeit pause start
zeit pause start --label lunch
zeit pause start --at 12:00 --label lunch
```

## End the current pause

```bash
zeit pause end
zeit pause end --at 12:30
```

Example:

```text
Ended pause at 2026-05-27 12:31:22 [lunch]
Duration: 00:31:04
```

## Edit a pause

The pause number is shown by `zeit report <DATE>`.

```bash
zeit pause edit 2026-05-27 1 --start 12:00 --end 12:30
zeit pause edit 2026-05-27 1 --label lunch
```

---

# Status

```bash
zeit status
```

Example:

```text
Started:  2026-05-27 09:01:03
Elapsed:  05:42:10
Pauses:   00:30:00
Worked:   05:12:10
Target:   08:00
Overtime: -02:47:50
```

If a pause is running, one more line is shown, and its time is not counted as worked:

```text
Pause:    [lunch] 00:12:41 (running)
```

---

# Reports

```bash
zeit report                 # all time, by year
zeit report 2026            # one year, by month
zeit report 2026-05         # one month, by week
zeit report 2026-05-27      # one day, session by session
zeit report today           # same as today's date
```

Reports aggregate:

* Worked time
* Pause time
* Overtime (worked time minus each session's daily target)
* Day totals

The daily report (`--month M --day D`) also shows each session's target.

The one-day report also shows each session's target and numbers its pauses:

```text
── 2026-05-27 sessions ──────────────────────────────────────────
  09:00:00 → 17:30:00   target 08:00   worked 08:00:00   pauses 00:30:00   overtime +00:00:00
    pause 1  [lunch]  00:30:00
─────────────────────────────────────────────────────────────
  2026-05-27 total     1 day(s)   worked 08:00:00   pauses 00:30:00   overtime +00:00:00
```

---

# Storage

All data lives in `~/Library/zeit/data.json` on macOS and in
`$XDG_CONFIG_HOME/zeit/data.json` (default `~/.config/zeit/data.json`) on Linux,
as human-readable JSON. No external services, no databases.

# Overtime definition

Every completed session is treated as one working day. The daily target
defaults to 8 hours and can be changed per session with `--target HH:MM`
on `session start` or later with `session edit <date> --target HH:MM`.
Overtime = worked time - that session's target. Sessions recorded before
per-session targets existed keep the 8-hour default.
