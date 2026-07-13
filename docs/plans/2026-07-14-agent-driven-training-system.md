# Exploration: Agent-driven training system

**Status:** Grilled 2026-07-14 — topology and boundaries decided (see Decisions). Next
artifact is the read-only context slice design.

## Vision (confirmed)

`training-cli` is used **only by agents** (Codex, Claude Code, Hermes, OpenClaw — never by
a human directly). The human uses `training-app`. The agent **controls the domain content
of the app**: the plan, goals, adaptations, and narratives the human reads are authored by
the coaching agent, with the CLI as the agent's hands.

```
 human (Sergio) ── phone ── Tailscale ──┐
                                        ▼
                             training-app on mini PC
                             (SvelteKit + SQLite = single source of truth)
                                        ▲
 coaching agent ── training-cli ── Tailscale/HTTP (role tokens)
 (Luna role: any harness holding the coach token)

 nutrition: fatsecret-cli (independent, local-first) ── combined by the agent, not the app
```

## Current assets (verified 2026-07-14)

| Asset | State |
|---|---|
| `training-cli` (this repo) | Rust CLI, SQLite at `TRAINING_CLI_HOME/training.db`. Final local-first release landed 2026-07-14 (exercise catalog, atomic `log`, review fixes; 18/18 tests). Becomes a thin HTTP SDK. |
| `training-app` (`../training-app`) | SvelteKit + better-sqlite3, running on the mini PC, reached via Tailscale from the gym. Rich domain layer: active workout, hidden forward plan, coaching decisions vs workout facts, goals, substitutions, coaching gateway. `CONTEXT.md` names **Luna** as coach, **Telegram** as dialogue channel. Only 3 browser-oriented API endpoints today. |
| GitHub issues #1–#3 | #1 temporal session contract; #2 cross-CLI nutrition skill; #3 Hermes nutrition-coaching PRD. All in scope of this exploration. |
| `agent/temporal-session-foundation` branch | Superseded (decision 8) — archive unmerged. |
| `fatsecret-cli` | Stays independent local-first; combined at the agent layer. |

## Decisions

| # | Decision | Rationale |
|---|---|---|
| 1 | **The app's DB is the single source of truth.** | The rich domain model (hidden plan, coaching decisions, lifecycle) already lives there; migrating it into Rust or maintaining sync would be larger and riskier. |
| 2 | **CLI accesses it via an agent-grade HTTP API** through the app's domain layer, with token auth separate from browser CSRF. | All writes obey the same invariants as the UI ("facts never silently rewritten"); no schema coupling. |
| 3 | **Agent control = domain content only.** Plan, goals, adaptations, substitutions, rationales, pending discussions, progress narratives. UI code/layout/pages stay engineering artifacts. | Clear trust boundary; the gym UI stays predictable and governed by DESIGN.md. |
| 4 | **training-cli becomes a thin agent SDK with no local DB.** One-time migration of `training.db` history into the app DB; the CLI keeps its ergonomic surface (shorthand log, context markdown, catalog search, JSON). | One truth, no sync code. CLI unavailable when the app is down — acceptable. |
| 5 | **Authority = role tokens, not harnesses.** One `coach` token (the Luna role — whoever holds it is Luna); `observer` tokens are read-only. | One coaching voice, enforced server-side, harness-agnostic. |
| 6 | **Telegram remains the coaching-dialogue channel.** The app stores compact pending-discussion records and committed state only; the coach token commits after confirmation. | Matches the existing app model; app is not a chat UI. |
| 7 | **Deployment: app server on the mini PC, reached via Tailscale** (already running this way). Agents connect over the tailnet with tokens from day one. | Always-on for gym hours; no new infrastructure. |
| 8 | **The temporal session contract (issue #1) moves into the app's schema/API** (`scheduled_at`/`started_at`/`ended_at`, timezone, state: planned/active/completed/missed). Archive `agent/temporal-session-foundation` unmerged. | Temporal truth belongs where the workout lives; the companion binary built it into the store being retired. |
| 9 | **Nutrition (issues #2/#3) is in scope** of this exploration, alongside training-cli. | Nutrition timing is the primary consumer of the temporal contract. |
| 10 | **Nutrition data combines at the agent layer.** fatsecret-cli stays independent; the coach reads both sides and combines; recommendations land in Telegram; only committed training decisions enter the app. | Two clean systems, no schema marriage, app product boundary intact. |
| 11 | **The pending local-first CLI work lands now** as the last local-first release. | Green and tested; catalog data and log semantics feed the one-time migration; clean main for the rewrite. |
| 12 | **First vertical slice: read-only context.** App GET endpoints (today's workout, recent sessions, goals) + observer token; CLI `context`/`last` served from the app over Tailscale. | Proves auth, transport, and the SDK shape with zero write risk. |

## Slice roadmap (draft — refine per slice)

1. **Read-only context slice** (decision 12): observer token, GET endpoints, app-backed
   `training context` / `training last`.
2. **Temporal contract slice**: session state machine + timestamps in app schema/API
   (issue #1), readable through slice 1's path.
3. **Coach write slice**: coach token; author/adapt active workout, goals, substitutions,
   pending discussions via CLI.
4. **Migration + cutover**: one-time import of `training.db` history (incl. exercise
   catalog) into the app DB; CLI drops rusqlite; retire `training.db`.
5. **Nutrition coaching slice** (issues #2/#3): Hermes/Luna skill combining app context +
   fatsecret-cli for meal-timing recommendations in Telegram.

## Open questions (deferred, not blocking slice 1)

- Token issuance/storage mechanics on the tailnet (env var? config file? Tailscale identity?).
- Exact GET payload shapes — reuse the CLI's existing markdown-context and JSON formats as
  the starting contract?
- In-gym human writes keep flowing through the app UI as workout facts (unchanged), but the
  fact/decision split should be re-verified once the coach write slice lands.
- Multi-user is out; single user (Sergio) assumed throughout.
- What "controls the content" means for notifications/reminders, if any, is unexplored.

## Next steps

1. Land pending CLI work (decision 11) — done 2026-07-14.
2. Design doc for the read-only context slice (API contract + token scheme) in
   `training-app`, referencing this exploration.
3. Implement slice 1, then re-grill before the coach write slice.
