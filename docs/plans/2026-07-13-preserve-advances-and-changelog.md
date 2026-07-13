# Preserve current advances and add changelogs

**Planning model:** Terra low

**Execution model:** Luna low

**Execution mode:** Preserve-first. Do not amend, rebase, force-push, reset, clean, stash, or rewrite existing history. Make only new additive commits after the checks below pass.

## Goal (measurable)

Preserve every intentionally versionable advance currently present in the two working trees in ordinary commits pushed to each repository's `origin/main`, while leaving secrets, databases, dependencies, and generated output untracked.

At completion:

1. `training-cli` contains a tracked root `CHANGELOG.md`, this plan, and the already-existing untracked plan `docs/plans/2026-07-07-svelte-training-app.md` in a new commit on top of `06490e6`.
2. `training-app` contains a tracked root `CHANGELOG.md` in a new commit on top of `802ea45`.
3. Neither commit includes `.env.local`, any `.env.*` secret, SQLite/DB files, `node_modules/`, `target/`, `.svelte-kit/`, `build/`, `.output/`, or other generated artifacts.
4. Rust tests/build and Svelte checks/tests/lint/build have been run with their real exit status recorded in the commit/PR notes or execution report. A failure stops the relevant commit unless the user explicitly accepts it.
5. After pushing, local `HEAD` and `origin/main` resolve to the same commit in each repository, and `git status --short --branch` shows no tracked or untracked versionable work left behind. Ignored local operational files may remain ignored.

## Inspected baseline (2026-07-13)

### `/home/scvm/work/cli/training-cli`

- Public remote: `https://github.com/sergio-cvm00/training-cli.git`.
- Local branch: `main`, currently equal to `origin/main` at `06490e6e183d7e2fcf1d7a180d49f0e68af9bb32`.
- The only discovered versionable working-tree item is untracked: `docs/plans/2026-07-07-svelte-training-app.md`.
- `target/` is ignored. `.gitignore` also excludes `.venv/`, Python caches, and `.training/` (including a project-local SQLite database/config/exports).
- No existing tracked `CHANGELOG.md` or tracked `docs/` files exist.
- The Rust project exposes `cargo test` and `cargo build --release` as the documented verification commands.

### `/home/scvm/work/web/training-app`

- Private remote: `https://github.com/Sergio-CVM00/training-app.git`.
- Local branch: `main`, currently equal to `origin/main` at `802ea45cff53372a7d77198d55cee2807380fe97`.
- No tracked or untracked source/documentation changes were discovered. The existing application, its tests, plans, ADRs, server-side SQLite integration, routes, and UI are already contained in the initial snapshot commit and are already on the remote.
- Ignored local files/directories discovered: `.env.local`, `.svelte-kit/`, and `node_modules/`.
- `.gitignore` also excludes `.env`, `.env.*` (except approved examples), `.output/`, deployment directories, `/build`, and `node_modules`.
- `package.json` provides `npm run check`, `npm test`, `npm run lint`, and `npm run build`.

## Architecture and scope

This is a release-documentation and preservation operation, not a feature implementation or data migration.

- `training-cli` remains the Rust local-first workout-memory CLI. Its existing untracked Svelte-app implementation plan is documentation and must be preserved as such; it is not evidence that the proposed app work was implemented in this repository.
- `training-app` remains the SvelteKit UI/API layer whose committed source snapshot contains the current implemented app work. Its runtime SQLite data remains external/local and must never enter Git.
- Each repository receives its own root-level `CHANGELOG.md`; do not duplicate application source across repositories or move the SQLite database.
- The CLI changelog is the required place to reference `Sergio-CVM00/training-cli` issues `#1`, `#2`, and `#3`. It must use links and explicit non-resolution language. The app changelog describes the already-implemented application snapshot without claiming that it closes any CLI issue.
- Do not modify source code, schemas, package lockfiles, Cargo lockfiles, application plans, configuration, Git remotes, branch names, or history as part of this operation.

## Ordered execution steps

### 1. Establish a safe, current baseline in both repositories

Run the following independently in each repository before editing. Use `git fetch --prune origin` only to refresh remote-tracking metadata; it must not alter the working tree or history.

```bash
cd /home/scvm/work/cli/training-cli
git status --short --branch
git fetch --prune origin
git rev-parse HEAD
git rev-parse origin/main
git log --oneline --decorate -5
git diff --check
git diff --cached --check
git status --short --untracked-files=all --ignored=matching

cd /home/scvm/work/web/training-app
git status --short --branch
git fetch --prune origin
git rev-parse HEAD
git rev-parse origin/main
git log --oneline --decorate -5
git diff --check
git diff --cached --check
git status --short --untracked-files=all --ignored=matching
```

Expected baseline: both branches are based on their current remote `main`; CLI has only the existing untracked July 7 plan plus ignored `target/`; app has only ignored local operational paths.

**Stop condition:** Stop before editing if either worktree has unexpected modified/staged/untracked versionable paths, if `HEAD` and `origin/main` have diverged, or if `git diff --check` reports whitespace errors. Report the exact paths/state and ask the user whether to preserve them in this release or leave them untouched.

### 2. Inspect the referenced public CLI issues; do not change them

Retrieve the current title, body, labels, and state for `#1`, `#2`, and `#3` from `Sergio-CVM00/training-cli` using GitHub's UI, `gh issue view`, or a reviewed GitHub API response. Record the issue titles accurately before composing the CLI changelog.

Example read-only commands:

```bash
cd /home/scvm/work/cli/training-cli
gh issue view 1 --repo Sergio-CVM00/training-cli --json number,title,state,url,labels,body
gh issue view 2 --repo Sergio-CVM00/training-cli --json number,title,state,url,labels,body
gh issue view 3 --repo Sergio-CVM00/training-cli --json number,title,state,url,labels,body
```

Do not close, comment on, relabel, or otherwise mutate the issues.

**Stop condition:** If issue lookup is unavailable, do not guess titles or status. The changelog may still include only the verified permanent links `https://github.com/Sergio-CVM00/training-cli/issues/1`, `/2`, and `/3`, and must say their resolution status was not changed by this preservation commit.

### 3. Create the CLI changelog and preserve the two documentation plans

Create exactly:

- `/home/scvm/work/cli/training-cli/CHANGELOG.md`
- `/home/scvm/work/cli/training-cli/docs/plans/2026-07-13-preserve-advances-and-changelog.md` (this plan; already created during planning)

Preserve and stage exactly the existing documentation file:

- `/home/scvm/work/cli/training-cli/docs/plans/2026-07-07-svelte-training-app.md`

Use an additive Keep a Changelog-style entry headed `## [Unreleased] - 2026-07-13` (or a clearly marked preservation snapshot equivalent). Its required structure is:

```md
# Changelog

All notable versioned changes to this project are documented in this file.

## [Unreleased] - 2026-07-13

### Implemented in the repository
- [Fact-based summary of the currently committed CLI capabilities from README/source/tests.]
- [Fact-based summary that agent skills and contributor guidance are present.]
- Added the preserved Svelte training-app implementation plan under `docs/plans/`; it is planning documentation, not an implementation claim.

### Related issues — still tracked separately
- [#1](https://github.com/Sergio-CVM00/training-cli/issues/1) — [verified issue title, if available]. This commit preserves/documentates current work; it does not resolve or close this issue.
- [#2](https://github.com/Sergio-CVM00/training-cli/issues/2) — [verified issue title, if available]. This commit preserves/documentates current work; it does not resolve or close this issue.
- [#3](https://github.com/Sergio-CVM00/training-cli/issues/3) — [verified issue title, if available]. This commit preserves/documentates current work; it does not resolve or close this issue.

### Pending / not represented as completed
- Work described by the related issues remains separately tracked until verified and explicitly closed.
- The Svelte training-app plan describes proposed work; do not represent it as implemented by `training-cli`.
```

Do not use closing keywords (`Fixes`, `Closes`, `Resolves`) in this changelog, commit subject/body, or push message. Do not invent issue implementation status from issue titles.

Stage with a path allowlist, never `git add -A`:

```bash
cd /home/scvm/work/cli/training-cli
git add -- CHANGELOG.md \
  docs/plans/2026-07-07-svelte-training-app.md \
  docs/plans/2026-07-13-preserve-advances-and-changelog.md
git diff --cached --check
git diff --cached --name-only
git diff --cached --stat
```

Required staged-path result: only the three paths above.

**Stop condition:** Unstage and stop if the staged path list contains any `.env*`, credential, `*.db`, `*.sqlite*`, `.training/`, `node_modules/`, `target/`, `build/`, `.svelte-kit/`, `.output/`, lockfile, source file, or other unreviewed path. Use `git restore --staged -- <unexpected-path>` only to unstage the accidental addition; do not delete user files.

### 4. Verify the CLI before committing

Run the documented checks without changing tracked files:

```bash
cd /home/scvm/work/cli/training-cli
cargo test
cargo build --release
git diff --cached --check
git status --short --branch
```

`target/` created or updated by the build must remain ignored and unstaged.

**Stop condition:** If either Cargo command fails, do not commit. Preserve the staged documentation exactly as-is, report the real failure output, and request a decision. Do not mask failures by deleting tests, changing code, or committing with a known red build.

### 5. Commit and push the CLI preservation snapshot

After Step 4 passes and the index contains only the allowlisted paths:

```bash
cd /home/scvm/work/cli/training-cli
git commit -m "docs: preserve training app plan and changelog"
git push origin main
git ls-remote --heads origin main
git rev-parse HEAD
git rev-parse origin/main
git status --short --branch
```

The commit body, if one is used, must state that issue resolution was not changed; it must contain no closing keywords.

**Stop condition:** If `git push` rejects because remote `main` advanced, stop. Fetch and inspect the new commits; do not pull with rebase, merge, force-push, or amend without a new explicit decision.

### 6. Create the app changelog without capturing runtime state

Create exactly:

- `/home/scvm/work/web/training-app/CHANGELOG.md`

Use a dated preservation entry with explicit status boundaries:

```md
# Changelog

All notable versioned changes to this project are documented in this file.

## [Unreleased] - 2026-07-13

### Implemented in the repository
- Added the committed SvelteKit training UI and server-side training-data integration.
- Added the committed workout, exercise history, weekly view, progress, and goal routes.
- Added committed domain logic and automated tests for planning, recommendations, workout lifecycle, recording state, goals, coaching boundaries, and repository access.
- Added committed product documentation, ADRs, and UX/design guidance.

### Preserved current state
- The implementation above was already present in the `802ea45` snapshot and is confirmed on `origin/main`.

### Pending / not represented as completed
- This changelog does not claim deployment, cross-device synchronization, data migration, or issue resolution.
- Local runtime configuration and training data remain outside Git.
```

Adapt wording only after reviewing the tracked files; retain the three status headings. Do not claim live deployment, completed device sync, or persistence of database contents. Do not add an issue-closing reference.

Before staging, explicitly inspect the candidate and the ignore rules:

```bash
cd /home/scvm/work/web/training-app
git check-ignore -v .env.local node_modules .svelte-kit || true
git add -- CHANGELOG.md
git diff --cached --check
git diff --cached --name-only
git diff --cached --stat
```

Required staged-path result: only `CHANGELOG.md`.

**Stop condition:** Unstage and stop if any local runtime file, secret, database, dependency directory, generated directory, build output, source change, or unreviewed file appears in the index. Do not use `git clean` or delete ignored files.

### 7. Verify the app before committing

Use the checked-in dependencies; do not run an installation that could rewrite `package-lock.json`:

```bash
cd /home/scvm/work/web/training-app
npm run check
npm test
npm run lint
npm run build
git diff --cached --check
git status --short --branch
```

The build may update ignored `.svelte-kit/` and create ignored output. Confirm none of it is staged:

```bash
git status --short --untracked-files=all --ignored=matching
git diff --cached --name-only
```

**Stop condition:** If any check/test/lint/build command fails, do not commit. Report the actual command and failure output. If dependency installation is required because `node_modules/` is missing, stop and request permission before any install; do not alter the lockfile as part of this preservation task.

### 8. Commit and push the app changelog

After Step 7 passes and only `CHANGELOG.md` is staged:

```bash
cd /home/scvm/work/web/training-app
git commit -m "docs: add implementation changelog"
git push origin main
git ls-remote --heads origin main
git rev-parse HEAD
git rev-parse origin/main
git status --short --branch
```

**Stop condition:** Treat a push rejection or remote advancement exactly as in Step 5: stop and inspect; do not rewrite history or force-push.

### 9. Final cross-repository audit

Run and retain the output:

```bash
for repo in \
  /home/scvm/work/cli/training-cli \
  /home/scvm/work/web/training-app
do
  echo "=== $repo ==="
  cd "$repo"
  git status --short --branch
  git log -1 --oneline
  git rev-parse HEAD
  git rev-parse origin/main
  git diff --check
  git diff --cached --check
  git ls-files | grep -E '(^|/)(\.env($|\.)|.*\.(db|sqlite|sqlite3)$|node_modules|target|build|\.svelte-kit|\.output)(/|$)' || true
done
```

Also inspect the pushed files through the GitHub repository view (or `git show origin/main:CHANGELOG.md`) to verify that the CLI changelog links `#1`, `#2`, and `#3`, says they remain separately tracked, and does not claim they are resolved.

## Acceptance criteria

- Both repositories have a new, non-rewritten commit on `main` and the pushed SHA equals local `HEAD`.
- CLI commit includes exactly `CHANGELOG.md`, the existing July 7 plan, and this July 13 plan; app commit includes exactly its `CHANGELOG.md`.
- All required verification commands pass with real output, or execution stops before any affected commit.
- CLI `CHANGELOG.md` contains valid links to all of `#1`, `#2`, and `#3`, clearly separates “Implemented” from “Pending”, and explicitly says the commit does not resolve/close them.
- App `CHANGELOG.md` clearly separates the committed snapshot from pending deployment/sync/data migration work.
- No secret, `.env.local`, database, local training record, dependency directory, target directory, build output, or generated artifact is tracked or staged.
- Existing commits are untouched: no amend, reset, rebase, force push, stash, clean, history rewrite, or issue mutation.

## Global stop conditions

Stop immediately and report rather than improvising if any of the following occurs:

- a new unreviewed versionable file or existing local modification is discovered;
- a candidate file may contain a credential, private configuration, database, personal workout data, or generated content;
- local and remote `main` diverge or the remote advances during execution;
- a test/check/lint/build fails;
- GitHub authentication/authorization cannot push to either repository;
- the actual issue text cannot support an implementation claim;
- the requested action would require modifying source, runtime data, issue state, configuration, or existing Git history.

In every stop case, leave working files intact, do not make partial commits, and provide the exact command output and the smallest decision needed from the user.
