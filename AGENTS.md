# Agent Instructions for training-cli

This repository contains `training-cli`, a local-first workout logging CLI.

## Main Goal

Build and maintain a reliable CLI for saving, updating, deleting, querying, importing, and exporting training logs.

## Product Boundary

This is not a mobile app, nutrition tracker, or full AI coach. It is a structured training memory layer.

## Data Safety

Never delete or overwrite user workout data without explicit user instruction.

Prefer CLI commands and repository APIs over direct SQLite manipulation.

## Verification

Before completing any change, run:

```bash
training --help
training init --local
training log "Bench Press: 80x8@8, 80x7@8.5"
training last "Bench Press"
training context --last 4weeks --format markdown
```

Also run the test suite.

## Important Constraints

- Local-first storage.
- SQLite as default database.
- Human-readable CLI output.
- Machine-readable JSON output for the agent write seam: `training log --json --command-id <id>`.
- Reuse a command id only for a retry of identical input; never retry a write with a new id.
- Markdown context output for agents.
- Simple schema.
- Reliable CRUD.
