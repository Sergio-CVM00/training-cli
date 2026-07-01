---
name: training-session-logger
description: Use when the user wants to log, record, capture, normalize, clean up, or review a workout session entry for training-cli. Focuses on preserving complete session/set/program facts, separating missing fields from supplied data, and avoiding coaching recommendations unless another skill is invoked.
---

# Training Session Logger

Use this skill to turn a user-described workout into a complete, consistent training log entry. Logging is fact capture, not coaching.

## Grounding

For logging schema and vocabulary, read:

```txt
wiki/01-core-concepts.md
wiki/12-data-and-derived-metrics.md
wiki/07-pain-form-guardrails.md
```

If the log entry will be used for a recommendation in the same turn, also use `training-logbook` or follow `AGENTS.md` decision rules before recommending.

Completion criterion: the log entry uses repo vocabulary, preserves the user's facts, and does not contradict the wiki.

## Capture Order

Extract facts in this order:

```txt
session-level facts
exercise entries
set-level facts
program-level facts
explicit constraints and notes
missing critical fields
```

Session-level facts:

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

Set-level facts:

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

Program-level facts:

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

Completion criterion: every fact present in the user's message is captured exactly once under the right level.

## Missing Fields

Do not invent missing values. Mark absent critical fields as missing, especially:

```txt
date
exercise name
load
reps
RPE or RIR
pain rating
form rating
recovery or energy signal
range-of-motion note for constrained movements
```

If the user provides partial set shorthand, preserve what is known and name what is unknown.

Example:

```txt
Input: "Bench 80kg x 8 @8, 80kg x 7 @8.5"
Known: exercise=Bench, load=80kg, reps=[8,7], RPE=[8,8.5]
Missing: date, set type, pain rating, form rating, recovery/energy signal
```

Completion criterion: missing fields are explicit and no placeholder is treated as a fact.

## Output Shape

Return logging output in this shape unless the user asks for a different format:

```txt
Session facts:
Exercise log:
Missing fields:
Notes:
```

Under `Exercise log`, group sets by exercise and preserve set order. Label warm-ups, working sets, drop sets, and backoff sets only when the user supplied that distinction.

Under `Notes`, include only user-supplied notes and non-coaching normalization notes, such as "RPE supplied instead of RIR" or "range of motion not specified for knee-sensitive movement."

Completion criterion: the output is ready to be entered into `training-cli` or handed to a logging command without adding invented facts.

## Boundaries

Do not turn a logging task into a progression recommendation. If the user asks what to do next, switch to `training-logbook` behavior: retrieve history, inspect guardrails, and use the recommendation output contract.

Do not count hard sets when effort data is missing. If RPE/RIR is present, hard-set interpretation may be labeled as an interpretation, not a logged fact.

Do not normalize pain or form ratings beyond the wiki scale. If the user gives free-text pain or form notes instead of ratings, preserve the note and mark the rating as missing.
