---
name: training-logbook
description: Use when logging workouts, retrieving workout history, reviewing progression, checking deload or fatigue signals, applying pain/form guardrails, or making training-cli coaching recommendations. Enforces complete logged facts, required history retrieval, wiki guardrails, and the standard recommendation output.
---

# Training Logbook

Use this skill to keep training facts and coaching decisions consistent. The wiki is the contract; this skill is the process for using it.

## Grounding

Before changing training logic or making recommendations, read the required files in `AGENTS.md`, especially:

```txt
wiki/README.md
wiki/00-agent-training-principles.md
wiki/01-core-concepts.md
wiki/03-progression-models.md
wiki/04-autoregulation-rpe-rir.md
wiki/05-volume-intensity-frequency.md
wiki/06-deload-fatigue-management.md
wiki/07-pain-form-guardrails.md
wiki/09-agent-decision-rules.md
wiki/11-evidence-map.md
wiki/12-data-and-derived-metrics.md
wiki/13-rule-model-drafts.md
```

Completion criterion: the relevant wiki rules have been read for the decision being made, and no coaching statement contradicts them.

## Logging Branch

When the user logs a workout, preserve facts separately from interpretations.

Capture session-level facts when available:

```txt
date
session title
session RPE
duration
bodyweight
energy rating
recovery rating
sleep
notes
```

Capture set-level facts when available:

```txt
exercise name or ID
set type
load
reps
RPE or RIR
pain rating
form rating
rest time
range-of-motion note when relevant
```

Capture program-level facts when available:

```txt
goal
current block
block type
week number
target rep range
target RPE/RIR range
progression model
explicit constraints
```

Do not invent missing values. If the user omits critical facts, record the known facts and list the missing fields. Critical fields include exercise history, current block, target rep range, RPE or RIR, pain rating, form rating, and recovery signal.

Completion criterion: every supplied fact is represented, missing critical facts are named, and interpretations are clearly labeled as interpretations.

## Retrieval Branch

When the user asks for a next target, progression, deload, pain/form decision, weekly review, or any coaching recommendation, retrieve logged data before deciding.

Prefer these commands when the `training` CLI is available:

```bash
training context --last 4weeks --format markdown
training last "<exercise>"
training history "<exercise>" --last 8weeks
```

Retrieve in this order:

```txt
current block and explicit constraints
last 2-3 exposures of the exercise
RPE/RIR, pain, form, recovery, weekly hard sets, and fatigue flags
4-8 weeks of relevant history for progression, fatigue, deload, or pain/form questions
```

If the CLI is unavailable or no data exists, say so. Use only facts the user provided or files you can inspect; do not fabricate history.

Completion criterion: the decision has current block/context, recent exercise exposures, and relevant guardrail data, or explicitly states which required data is missing.

## Recommendation Branch

Before recommending progression, apply this decision order:

```txt
read current block and explicit constraints
inspect the last 2-3 exercise exposures
inspect RPE/RIR, pain, form, recovery, weekly hard sets, and fatigue flags
apply the exercise progression model
choose the smallest viable change
explain the decision using the output standard
```

Never recommend aggressive progression when:

```txt
pain rating > 0
form rating <= 2
recovery <= 2
RPE is rising while performance is falling
the current block is a deload
critical exercise history, pain/form, RPE/RIR, or block data is missing
```

Use this exact output shape for recommendations:

```txt
Last performance:
Decision:
Reason:
Next target:
Guardrail:
```

The reason must cite the logged facts and derived interpretations that drove the decision. If critical data is missing, the default decision is repeat, hold, reduce, or ask for the missing data; not load or volume progression.

Completion criterion: the recommendation uses logged data when available, chooses the smallest viable change, respects all guardrails, and includes every output field.
