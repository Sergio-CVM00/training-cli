# training-cli

`training-cli` is a tiny local workout memory for your terminal.

It exists for one reason: keep training logs structured enough that you and AI coaching agents can actually use them later. No app account, no cloud sync, no dashboard ceremony. Just a fast native CLI, a local SQLite database, and clean exports.

## Why use it?

Most workout notes are easy to write and hard to use later. Chat logs, Notes.app entries, and random spreadsheets lose the details an agent needs to answer questions like:

- What did I do last time for bench press?
- Has my walking/cardio volume changed this week?
- Which sets had pain or poor form?
- What context should an AI coach read before recommending the next session?

`training-cli` turns this:

```txt
Bench Press: 80x8@8, 80x7@8.5
Walking at 7 km/h for 15 min
```

into structured local data that can be queried, corrected, and exported.

## What it does

- Logs strength work quickly from compact text.
- Stores workouts, exercises, and sets in SQLite.
- Tracks cardio fields like duration, distance, speed, pace, elevation, heart rate, calories, and steps.
- Lets you update or delete bad logs.
- Shows the last session or history for an exercise.
- Exports deterministic Markdown/JSON context for AI agents.
- Runs as a small Rust binary with low startup overhead.

## Install from source

```bash
git clone https://github.com/Sergio-CVM00/training-cli.git
cd training-cli
cargo build --release
```

The binary will be at:

```bash
target/release/training
```

You can copy it somewhere on your `PATH`, or run it through Cargo while developing:

```bash
cargo run -- --help
```

## Quick start

Initialize local storage:

```bash
training init
```

Or keep data inside the current project folder:

```bash
training init --local
```

Log strength work:

```bash
training log "Bench Press: 80x8@8, 80x7@8.5, 75x9@8"
```

Log cardio:

```bash
training add workout \
  --date today \
  --title "Walking at 7 km/h" \
  --type cardio \
  --duration-min 15 \
  --speed-kmh 7
```

See your last bench session:

```bash
training last "Bench Press"
```

Export agent-readable context:

```bash
training context --last 4weeks --format markdown > TRAINING_CONTEXT.md
```

Export all data as JSON:

```bash
training export --format json
```

## Data location

By default:

```txt
~/.training-cli/
  training.db
  config.json
  exports/
```

Project-local mode:

```txt
.training/
  training.db
  config.json
  exports/
```

## Example output

```txt
Bench Press - Last Session
Date: 2026-07-01
Workout: Upper A

1. 80kg x 8 @ RPE 8
2. 80kg x 7 @ RPE 8.5
3. 75kg x 9 @ RPE 8
```

```txt
Workout: Walking at 7 km/h
Date: 2026-07-01
Type: cardio
Cardio: 15 min, 7 km/h
```

## Current scope

This is intentionally not a full fitness platform. It does not do nutrition tracking, cloud sync, social features, wearables, mobile apps, or automatic programming.

It is a local training memory layer: write structured logs fast, query them later, export clean context when an AI agent needs to reason from facts instead of chat history.

## Development

```bash
cargo test
cargo build --release
```

`cargo fmt` requires the `rustfmt` component for your Rust toolchain.
