---
name: training-progression-review
description: Use when deciding next-session targets, progression eligibility, load/rep/set changes, deload need, fatigue flags, or hold/reduce decisions from training-cli history. Focuses on retrieving recent exercise context, applying progression guardrails, and producing the standard last-performance/decision/reason/next-target/guardrail output.
---

# Training Progression Review

Use this skill to decide whether the next training exposure should progress, repeat, hold, reduce, or deload. Review is data interpretation, not session logging.

## Grounding

Before reviewing progression, read:

```txt
wiki/README.md
wiki/00-agent-training-principles.md
wiki/03-progression-models.md
wiki/04-autoregulation-rpe-rir.md
wiki/05-volume-intensity-frequency.md
wiki/06-deload-fatigue-management.md
wiki/07-pain-form-guardrails.md
wiki/09-agent-decision-rules.md
wiki/12-data-and-derived-metrics.md
wiki/13-rule-model-drafts.md
```

Completion criterion: the review uses the wiki's progression, fatigue, pain/form, and missing-data rules without contradiction.

## Retrieve Context

Use logged data before deciding. Prefer these commands when the `training` CLI is available:

```bash
training context --last 4weeks --format markdown
training last "<exercise>"
training history "<exercise>" --last 8weeks
```

Retrieve in this order:

```txt
current goal, block, week, and explicit constraints
exercise prescription: target reps, target RPE/RIR, progression model
last 2-3 exposures for the exercise
4-8 weeks of relevant history for fatigue, pain, deload, or volume questions
```

If the CLI is unavailable or history is missing, say so and use only facts supplied by the user or available files.

Completion criterion: either the review has block/prescription/recent exposure data, or it explicitly names the missing data that limits the decision.

## Interpret Data

Separate logged facts from interpretations.

Logged facts include:

```txt
sets, reps, load, date
RPE or RIR
pain rating
form rating
readiness, recovery, energy
current block and explicit constraints
```

Derived interpretations include:

```txt
top-set trend
estimated 1RM trend
RPE drift
weekly hard sets
failure exposure
fatigue flags
progression eligibility
```

Do not treat an interpretation as a fact. Do not infer pain, form, recovery, block type, or target rep range when absent.

Completion criterion: the reason cites the facts and labels any derived interpretation driving the decision.

## Apply Gates

Check these gates before recommending any progression:

```txt
target reps achieved
RPE <= target max
pain == 0 for load progression
form >= 3
recovery >= 3
current block allows progression
no positive RPE drift with falling performance
no deload block
critical data present
```

Choose the smallest viable change:

```txt
add 1 rep before load when inside a rep range
use the smallest available load jump when load progression is eligible
add sets only when performance is stable or improving and recovery supports it
use technical progression when form is the limiter
hold or reduce when pain, poor form, low recovery, fatigue, or missing data blocks progression
```

Consider reduced volume or deload when two or more fatigue signs appear:

```txt
lower reps at same load
higher RPE at same reps/load
low energy
low recovery
poor form
repeated pain
high failure exposure
```

Completion criterion: every blocking gate is accounted for before any progression is recommended.

## Output Contract

Use this exact shape:

```txt
Last performance:
Decision:
Reason:
Next target:
Guardrail:
```

`Decision` must be one of:

```txt
increase load
add reps
add sets
repeat
hold
reduce load
reduce volume
deload
modify or substitute
ask for missing data
```

If critical data is missing, default to repeat, hold, reduce, or ask for missing data. Do not recommend aggressive load or volume progression.

Completion criterion: the output includes all five fields, states one concrete next target, and names the guardrail that would change the decision next time.

## Boundaries

If the user is trying to log a new workout, use `training-session-logger` behavior first.

If the request combines logging and recommendation, log the facts first, then run this review on the logged facts and retrieved history.

Do not provide medical diagnosis or treatment. Pain and form rules are conservative decision-support guardrails.
