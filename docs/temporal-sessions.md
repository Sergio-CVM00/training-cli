# Temporal sessions

This vertical slice adds the `training-schedule` companion binary. It uses the same SQLite database and path-resolution rules as `training` while the monolithic CLI is prepared for later command consolidation.

## Build

```bash
cargo build --release
```

Cargo discovers `src/bin/training-schedule.rs` automatically and produces:

```text
target/release/training-schedule
```

## Create a planned session

```bash
training-schedule add \
  --starts-at "2026-07-13T18:30:00+02:00" \
  --title "Lower A" \
  --type strength \
  --duration-min 75 \
  --target-rpe 8 \
  --focus lower_body
```

Timestamps must use RFC 3339 with an explicit offset.

## Lifecycle

```bash
training-schedule list --status planned
training-schedule start <session-id>
training-schedule complete <session-id> --workout-id <workout-id>
training-schedule cancel <session-id>
```

Valid transitions are:

```text
planned -> in_progress
planned -> completed
planned -> cancelled
in_progress -> completed
```

## Agent context

```bash
training-schedule context --at "2026-07-13T17:00:00+02:00"
```

The JSON response exposes:

- current in-progress session;
- previous completed session;
- next planned session;
- minutes until the next session;
- minutes since the previous session;
- completeness flags for duration and target intensity.

The command accepts an explicit `--at` instant so tests and agent decisions remain deterministic.

## Storage

The binary creates a `scheduled_sessions` table in the existing `training.db`. It respects:

- `TRAINING_CLI_LOCAL=1` for `.training/training.db`;
- `TRAINING_CLI_HOME` for a custom root;
- the existing `.training` directory when present;
- `~/.training-cli/training.db` otherwise.

The temporal table does not modify historical `workout_sessions`. Completed scheduled sessions may reference an existing workout through `--workout-id`.

## Current boundary

This slice establishes the temporal contract without changing the existing `training` command surface. A follow-up can move the implementation behind `training schedule ...` once `src/main.rs` is modularized. Nutrition or carbohydrate recommendations remain outside this binary and belong to the coordinating agent skill.
