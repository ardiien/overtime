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

Available commands:

| Command  | Description                    |
|----------|--------------------------------|
| `record` | Start or end work sessions     |
| `pause`  | Start or end pauses            |
| `status` | Show the active session status |
| `report` | Generate overtime reports      |
| `edit`   | Edit sessions or pauses        |
| `delete` | Delete sessions by date        |

---

# Recording Work Sessions

## Start a session

```bash
zeit record start
```

Start a session at a specific time:

```bash
zeit record start --at 09:00
```

Start a session with a different daily target (`HH:MM`, default `8:00`):

```bash
zeit record start --target 4:00
zeit record start --at 09:00 --target 6:30
```

## End a session

```bash
zeit record end
```

End a session with a specific time:

```bash
zeit record end --at 17:30
```

When ending a session, `zeit` prints:

* Worked time
* Pause duration
* Overtime or undertime

Example:

```text
Session ended at 2026-05-27 17:42:10
Started:    2026-05-27 08:58:01
Pauses:     00:32:00
Worked:     08:12:09
Overtime:  +00:12:09
```

---

# Pauses

## Start a pause

```bash
zeit pause start
```

With a label:

```bash
zeit pause start --label lunch
```

## End the current pause

```bash
zeit pause end
```

Example:

```text
Pause ended at 2026-05-27 12:31:22 [lunch] - duration: 00:31:04
```

---

# Status

Show the currently active session:

```bash
zeit status
```

Example:

```text
Started:  2026-05-27 09:01:03
Elapsed:  05:42:10
Pauses:   00:30:00
Worked:   05:12:10
```

If a pause is active:

```text
Pause in progress: lunch - 00:12:41
```

---

# Reports

Generate overtime reports:

```bash
zeit report
```

Filter by year:

```bash
zeit report --year 2026
```

Filter by month:

```bash
zeit report --month 5
```

Filter by specific day:

```bash
zeit report --month 5 --day 27
```

Show today's entries:

```bash
zeit report now
```

Reports aggregate:

* Worked time
* Pause time
* Overtime (worked time minus each session's daily target)
* Day totals

The daily report (`--month M --day D`) also shows each session's target.

---

# Editing Sessions

## Edit a session

```bash
zeit edit record 2026-05-27 --start 09:00
```

Edit session end:

```bash
zeit edit record 2026-05-27 --end 17:45
```

Change the daily target of an existing session (`HH:MM`):

```bash
zeit edit record 2026-05-27 --target 4:00
```

## Edit a pause

```bash
zeit edit pause 2026-05-27 1 --start 12:00 --end 12:30
```

Rename a pause:

```bash
zeit edit pause 2026-05-27 1 --label lunch
```

---

# Deleting Sessions

Delete all sessions for a date:

```bash
zeit delete 2026-05-27
```

Skip confirmation prompt:

```bash
zeit delete 2026-05-27 --yes
```

---

# Storage

All data lives in `~/Library/zeit/data.json` as human-readable JSON.
No external services, no databases.

# Overtime definition

Every completed session is treated as one working day. The daily target
defaults to 8 hours and can be changed per session with `--target HH:MM`
on `record start` or later with `edit record <date> --target HH:MM`.
Overtime = worked time - that session's target. Sessions recorded before
this feature existed keep the 8-hour default.
