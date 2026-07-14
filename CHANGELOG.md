# Changelog

## Unreleased

- Add idempotent `training log --command-id` receipts with conflict detection.
- Add `training log --json` success, replay, and structured error envelopes.
- Preserve safety context by failing on unreadable configuration instead of loading defaults.
- Canonicalize catalog IDs to exercise names and emit UTC RFC 3339 timestamps.

All notable versioned changes to this project are documented in this file.

## [Unreleased] - 2026-07-13

### Implemented in the repository

- `training-cli` provides a local-first Rust workout memory CLI backed by local SQLite, with commands for initialization, configuration, workout and set logging, listing, history, context, and export.
- The repository includes portable agent skills for training session logging, training progression review, and the associated contributor/installation guidance.
- Added the preserved Svelte training-app implementation plan under `docs/plans/`; it is planning documentation, not an implementation claim in this repository.
- Preserved the current release-planning record in `docs/plans/2026-07-13-preserve-advances-and-changelog.md`.

### Related issues — still tracked separately

- [#1](https://github.com/Sergio-CVM00/training-cli/issues/1) — P0: Add temporal session contract for nutrition-aware coaching. This preservation commit documents current work; it does not resolve or close this issue.
- [#2](https://github.com/Sergio-CVM00/training-cli/issues/2) — P0: Add cross-CLI performance nutrition coaching skill. This preservation commit documents current work; it does not resolve or close this issue.
- [#3](https://github.com/Sergio-CVM00/training-cli/issues/3) — PRD: Cross-CLI training-aware nutrition coaching for Hermes. This preservation commit documents current work; it does not resolve or close this issue.

### Pending / not represented as completed

- Work described by the related issues remains separately tracked until verified and explicitly closed; this commit did not change their state.
- The Svelte training-app plan describes proposed work; it must not be represented as implemented by `training-cli`.
