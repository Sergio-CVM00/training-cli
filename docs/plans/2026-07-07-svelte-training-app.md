# Svelte Training App Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Build a mobile-first SvelteKit web UI for Sergio to view today’s planned workout, log gym sets quickly, and persist everything into the existing `training-cli` local SQLite data source so Hermes can query the same facts.

**Architecture:** Keep `training-cli` as the local training memory layer and build a SvelteKit app as a UI/API layer over the existing SQLite schema at `~/.training-cli/training.db`. Add app-owned planning/recommendation code in TypeScript, but do not duplicate workout logs into a second database. Store only facts in SQLite; keep progression/recommendations as derived data.

**Tech Stack:** SvelteKit, TypeScript, simple mobile-first CSS or Tailwind, SQLite access from server-only modules, Vitest for pure recommendation tests, Playwright or SvelteKit route tests for core API/UI flows.

---

## Inspection Findings

### Existing project

- Repo: `/home/scvm/work/cli/training-cli`
- Binary: `/home/scvm/work/cli/training-cli/target/release/training`
- Language: Rust
- Storage: SQLite via `rusqlite`
- Default DB path from README/source: `~/.training-cli/training.db`
- Current live DB exists: `~/.training-cli/training.db`
- Current live config exists: `~/.training-cli/config.json`

### Existing commands

`training --help` exposes:

```text
init
config
add
list
show
update
delete
log
last
history
context
export
```

Important agent-readable commands already available:

```bash
training last "Exercise Name" --json
training history "Exercise Name" --last 8weeks
training context --last 4weeks --format json
training export --format json
```

### Existing SQLite schema

The existing database already has the core MVP entities:

```sql
workout_sessions(
  id TEXT PRIMARY KEY,
  date TEXT NOT NULL,
  title TEXT,
  type TEXT NOT NULL DEFAULT 'gym',
  bodyweight_kg REAL,
  duration_min INTEGER,
  distance_km REAL,
  speed_kmh REAL,
  pace_min_per_km REAL,
  elevation_gain_m REAL,
  avg_heart_rate_bpm INTEGER,
  max_heart_rate_bpm INTEGER,
  calories INTEGER,
  steps INTEGER,
  perceived_energy INTEGER,
  perceived_recovery INTEGER,
  notes TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

exercise_logs(
  id TEXT PRIMARY KEY,
  workout_id TEXT NOT NULL,
  exercise_name TEXT NOT NULL,
  category TEXT,
  muscle_group_json TEXT,
  equipment TEXT,
  notes TEXT,
  sort_order INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (workout_id) REFERENCES workout_sessions(id) ON DELETE CASCADE
);

set_logs(
  id TEXT PRIMARY KEY,
  exercise_log_id TEXT NOT NULL,
  set_number INTEGER NOT NULL,
  set_type TEXT NOT NULL DEFAULT 'working',
  weight_kg REAL,
  reps INTEGER,
  target_reps INTEGER,
  rpe REAL,
  rir REAL,
  rest_sec INTEGER,
  tempo TEXT,
  form_rating INTEGER,
  pain_rating INTEGER,
  notes TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (exercise_log_id) REFERENCES exercise_logs(id) ON DELETE CASCADE
);
```

Indexes:

```sql
idx_workout_sessions_date ON workout_sessions(date)
idx_exercise_logs_name ON exercise_logs(exercise_name)
idx_set_logs_exercise_log_id ON set_logs(exercise_log_id)
```

### Integration decision

- **Data source:** Existing `training-cli` SQLite database.
- **DB path:** Default to `~/.training-cli/training.db`; allow override with `TRAINING_CLI_HOME` or `TRAINING_DB_PATH` for tests/dev.
- **Read strategy:** Read directly from SQLite in SvelteKit server-only modules. This gives fast structured queries for today/history/week/recommendations.
- **Write strategy:** Write directly to SQLite using the same schema and validations as `training-cli` where possible. Keep UUID IDs and timestamps compatible. Avoid shelling out to CLI for every set because gym logging must be fast and low-latency.
- **CLI compatibility:** Do not alter existing table names or column semantics. After each implemented write path, verify that `training export --format json` and `training context --format json` still read the data.
- **Risk:** `training-cli` currently has no explicit `status` field for planned/in_progress/completed/missed and no persistent program template tables. For MVP, derive “planned” workouts from TypeScript templates and store completed facts as normal `workout_sessions`, `exercise_logs`, and `set_logs`. Add columns/tables only after a migration decision.

---

## Sergio Training Rules to Encode

- Max 4 gym days/week.
- Usually morning sessions.
- Default 2 working sets to failure per exercise.
- Do not suggest extra volume by default.
- Priority muscles: chest, glutes, quads, hamstrings, back.
- Preferred exercises:
  - Peck deck
  - Incline barbell press
  - Machine shoulder press
  - Seated cable/plate row
  - Seated weighted crunch
  - Wall-sit over leg press
  - Pull-ups: 5-6 sets of 4-6 reps, to failure; add load if >6 reps.
- Avoid:
  - Leg curl due to knee
  - unnecessary leg press
  - excessive cable/pulley dependence if Basic Fit Lagoh is crowded
- Progression:
  - double progression
  - top of rep range achieved -> increase weight next exposure
  - reps fall/stall -> flag possible fatigue
  - repeated stagnation/fatigue -> deload recommendation

---

## Task 1: Create SvelteKit project shell

**Objective:** Create the app folder and baseline SvelteKit/TypeScript setup without touching training data.

**Files:**
- Create directory: `/home/scvm/work/web/training-app`
- Create/modify: generated SvelteKit files

**Step 1: Scaffold**

Run:

```bash
cd /home/scvm/work/web
npm create svelte@latest training-app
```

Select:

- SvelteKit minimal project
- TypeScript: yes
- ESLint/Prettier: yes if offered
- Vitest: yes if offered
- Playwright: optional but preferred

**Step 2: Install deps**

Run:

```bash
cd /home/scvm/work/web/training-app
npm install
npm install better-sqlite3 uuid
npm install -D @types/better-sqlite3 @types/uuid vitest
```

**Step 3: Verify baseline**

Run:

```bash
npm run check
npm test -- --run || true
npm run dev -- --host 0.0.0.0
```

Expected:

- `check` passes or shows only scaffold configuration issues to fix.
- Dev server starts.

**Commit:**

```bash
git add .
git commit -m "feat: scaffold training web app"
```

---

## Task 2: Add server-only DB path resolution with tests

**Objective:** Resolve the same training DB location as `training-cli` with test override support.

**Files:**
- Create: `src/lib/server/training/dbPath.ts`
- Create: `src/lib/server/training/dbPath.test.ts`

**Step 1: Write failing tests**

Test cases:

- `TRAINING_DB_PATH` wins when set.
- `TRAINING_CLI_HOME` resolves to `$TRAINING_CLI_HOME/training.db`.
- default resolves to `$HOME/.training-cli/training.db`.

**Step 2: Run RED**

```bash
npm test -- src/lib/server/training/dbPath.test.ts --run
```

Expected: fail because file/function does not exist.

**Step 3: Implement**

Implement:

```ts
export function resolveTrainingDbPath(env = process.env): string {
  if (env.TRAINING_DB_PATH) return env.TRAINING_DB_PATH;
  if (env.TRAINING_CLI_HOME) return `${env.TRAINING_CLI_HOME}/training.db`;
  if (!env.HOME) throw new Error('HOME is not set');
  return `${env.HOME}/.training-cli/training.db`;
}
```

**Step 4: Run GREEN**

```bash
npm test -- src/lib/server/training/dbPath.test.ts --run
npm run check
```

---

## Task 3: Add DB connection and read models

**Objective:** Read existing workout/exercise/set records from SQLite without modifying data.

**Files:**
- Create: `src/lib/server/training/db.ts`
- Create: `src/lib/training/types.ts`
- Create: `src/lib/server/training/repository.ts`
- Create: `src/lib/server/training/repository.test.ts`

**Repository API:**

```ts
export interface TrainingRepository {
  getWorkoutByDate(date: string): Promise<WorkoutWithExercises | null>;
  getExerciseHistory(exerciseName: string, limit: number): Promise<ExerciseHistoryEntry[]>;
  getWeeklySessions(from: string, to: string): Promise<WorkoutWithExercises[]>;
}
```

**TDD cases:**

- Creates a temp SQLite DB with the existing schema.
- Inserts one workout, one exercise, two sets.
- `getWorkoutByDate()` returns nested exercises/sets in sort order.
- `getExerciseHistory()` returns newest sessions first.

**Verification:**

```bash
npm test -- src/lib/server/training/repository.test.ts --run
npm run check
```

---

## Task 4: Add write paths for workout/exercise/set compatible with training-cli

**Objective:** Save workouts and sets from the web app into the existing tables in the same shape as `training-cli`.

**Files:**
- Modify: `src/lib/server/training/repository.ts`
- Modify: `src/lib/server/training/repository.test.ts`

**Repository additions:**

```ts
createWorkout(input: CreateWorkoutInput): Promise<Workout>;
createExercise(input: CreateExerciseInput): Promise<ExerciseLog>;
saveSet(input: SaveSetInput): Promise<SetLog>;
```

**TDD cases:**

- `createWorkout` writes UUID, date, title, type, created_at, updated_at.
- `createExercise` writes `muscle_group_json` as JSON array.
- `saveSet` auto-increments `set_number` per exercise.
- Invalid set with neither reps nor weight is rejected.

**Compatibility verification:**

After implementation, run a manual temp write and verify CLI can export it:

```bash
TRAINING_CLI_HOME=/tmp/training-web-compat training init
# point app tests or helper script at /tmp/training-web-compat/training.db
training export --format json --out /tmp/training-web-compat/export.json
```

---

## Task 5: Add Sergio program template module

**Objective:** Provide deterministic today workout templates without adding a complex program builder.

**Files:**
- Create: `src/lib/training/program.ts`
- Create: `src/lib/training/program.test.ts`

**Types:**

```ts
export type PlannedExercise = {
  name: string;
  category: string;
  muscleGroups: string[];
  equipment: string;
  targetSets: number;
  targetRepMin: number;
  targetRepMax: number;
  notes?: string;
};

export type WorkoutTemplate = {
  key: 'upper-a' | 'lower-a' | 'upper-b' | 'lower-b';
  title: string;
  exercises: PlannedExercise[];
};
```

**TDD cases:**

- Upper A includes Peck deck and incline barbell press.
- Pull-ups have 5-6 target sets and 4-6 reps.
- Leg curl is absent from all templates.
- No template suggests more than 2 sets by default except pull-ups.

---

## Task 6: Add progression recommendation module

**Objective:** Implement pure, tested double-progression logic.

**Files:**
- Create: `src/lib/training/recommendations.ts`
- Create: `src/lib/training/recommendations.test.ts`

**Output type:**

```ts
export type ExerciseRecommendation = {
  exerciseName: string;
  recommendedWeight: number | null;
  targetRepMin: number;
  targetRepMax: number;
  action: 'increase_weight' | 'increase_reps' | 'maintain' | 'deload' | 'review_fatigue';
  message: string;
};
```

**TDD cases:**

- If both working sets hit top of rep range -> `increase_weight`.
- If not yet at rep floor for all sets -> `maintain`.
- If reps improve but not top range -> `increase_reps`.
- If reps drop at same weight -> `review_fatigue`.
- If two repeated stalls or pain/form flags -> `deload`.
- Pull-ups >6 reps -> suggest adding load.
- Pull-ups <4 reps -> maintain/reduce difficulty, no load increase.

---

## Task 7: Add API routes

**Objective:** Expose server routes used by the Svelte UI.

**Files:**
- Create: `src/routes/api/workouts/today/+server.ts`
- Create: `src/routes/api/workouts/week/+server.ts`
- Create: `src/routes/api/workouts/[id]/start/+server.ts`
- Create: `src/routes/api/workouts/[id]/finish/+server.ts`
- Create: `src/routes/api/exercises/[name]/history/+server.ts`
- Create: `src/routes/api/exercises/[name]/recommendation/+server.ts`
- Create: `src/routes/api/sets/+server.ts`

**Routes:**

```text
GET /api/workouts/today
GET /api/workouts/week
POST /api/workouts/:id/start
POST /api/workouts/:id/finish
GET /api/exercises/:name/history
GET /api/exercises/:name/recommendation
POST /api/sets
```

**Important MVP constraint:** because the existing DB has no status column, `start` and `finish` may initially be no-ops or update `notes` conservatively only if needed. Do not add a status column until migration is intentionally designed.

---

## Task 8: Build Today screen

**Objective:** Mobile-first landing page showing the day’s workout and recommendations.

**Files:**
- Create/modify: `src/routes/+page.server.ts`
- Modify: `src/routes/+page.svelte`
- Create: `src/lib/components/ExercisePlanCard.svelte`

**Content:**

Each exercise card shows:

- Exercise name
- Target sets/reps
- Last performance
- Today recommendation
- Large tap target

Primary CTA:

```text
Start workout
```

---

## Task 9: Build workout logging flow

**Objective:** Let Sergio log sets quickly from mobile.

**Files:**
- Create: `src/routes/workout/[id]/+page.server.ts`
- Create: `src/routes/workout/[id]/+page.svelte`
- Create: `src/lib/components/SetLogger.svelte`

**UX requirements:**

- Weight defaults to recommendation.
- Reps input is one tap/selectable.
- Failure toggle defaults to true for working sets.
- RPE optional.
- Save set advances to next set.
- Default 2 working sets; pull-ups 5-6.

---

## Task 10: Build exercise history screen

**Objective:** Show last 5 sessions, best set, and current recommendation.

**Files:**
- Create: `src/routes/exercises/[name]/+page.server.ts`
- Create: `src/routes/exercises/[name]/+page.svelte`

**Display:**

```text
Incline barbell press

Last 5:
45 kg × 6, 5
42.5 kg × 8, 7
42.5 kg × 7, 6

Best set:
45 kg × 6

Recommendation:
Stay at 45 kg until both sets hit 6+ reps.
```

---

## Task 11: Build weekly overview

**Objective:** Show weekly completion, muscle coverage, and fatigue flags.

**Files:**
- Create: `src/routes/week/+page.server.ts`
- Create: `src/routes/week/+page.svelte`
- Create: `src/lib/training/weekly.ts`
- Create: `src/lib/training/weekly.test.ts`

**TDD cases:**

- Counts only `working` sets.
- Aggregates muscle groups from `muscle_group_json`.
- Flags priority groups below 8 sets.
- Does not count warmups toward weekly target.

---

## Task 12: Add Hermes query documentation

**Objective:** Document how Hermes should read the same data after app logging.

**Files:**
- Create: `docs/hermes-training-web-queries.md`

**Include:**

```bash
training context --last 4weeks --format json
training last "Press inclinado barra" --json
training history "Peck deck" --last 8weeks
sqlite3 ~/.training-cli/training.db 'SELECT ...'
```

Also document app API examples for local use:

```bash
curl http://localhost:5173/api/workouts/today
curl http://localhost:5173/api/workouts/week
```

---

## Final Verification Checklist

Before calling MVP complete:

- [ ] SvelteKit app runs locally.
- [ ] Mobile viewport shows Today screen clearly.
- [ ] Today workout loads with Sergio templates.
- [ ] Last performance is read from `~/.training-cli/training.db`.
- [ ] Logging a set writes to `set_logs`.
- [ ] Logging an exercise/workout remains readable by `training export --format json`.
- [ ] Exercise history shows last 5 sessions.
- [ ] Recommendation tests cover double progression and pull-ups.
- [ ] Weekly overview counts working sets by muscle group.
- [ ] Hermes can answer “revisa mi entreno” from the same DB.

---

## Do Not Build in MVP

- Accounts
- Public profiles
- Social sharing
- Cloud sync
- Nutrition tracking
- Complex charts
- AI inside the web app
- Native mobile app
- Stripe/payments
- Complex program builder
