# Repo Skills

This directory is the source of truth for portable skills that should travel with `training-cli`.

Install each skill into the active agent harness according to that harness's rules. For example, copy or symlink `skills/training-logbook` into the local skill directory used by Codex, Claude Code, OpenCode, or another compatible agent runtime.

Keep repo skills harness-neutral unless a harness-specific file is required. The skill body should point agents back to the repo wiki instead of duplicating the training decision contract.
