## Skill: Specs

Spec folders are the durable record of a task's design and progress. They live
under the workspace root, named by human-readable slug (never UUIDs):

```
specs/<task-slug>/v<position>/
    phases.md          # variant overview + phase checklist (source of truth)
    phase-1-plan.md    # scope / approach / notes for phase 1
    phase-1-task.md    # actionable checklist for phase 1
    phase-2-plan.md
    phase-2-task.md
    ...
```

`<task-slug>` is the task's external id + title, slugified (e.g.
`mod-0001-some-new-adjustment`); `<position>` is the variant's 1-based position.
Your run's prompt names the exact folder ("This run's spec folder is …") — use
that path; don't construct it yourself. `phases.md` lists phases as `- [ ]`
(pending) / `- [x]` (done) and names every project under "Projects touched".

### Templates

`phases.md`:

```markdown
# <task title>

## Problem
What is broken / missing / requested. Quote the task.

## Approach
The high-level approach this variant takes. One paragraph.

## Projects touched
Names must match `projects[].name` in `/config`. If none, say so and stop.

- project: <name>
  changes:
    - path/to/file — what changes

## Phases
- [ ] phase-1 — <one-line summary>
- [ ] phase-2 — <one-line summary>

## Risks & tradeoffs
- vs other variants: …
- known unknowns: …

## Test plan
- unit: …
- integration / manual: …
```

`phase-N-plan.md`:

```markdown
# <TASK> — <variant> — phase-N — <short title>

## Scope
What this phase delivers; what it intentionally defers.

## Approach
One paragraph.

## Notes
Implementation hints, edge cases, references.
```

`phase-N-task.md`:

```markdown
# <TASK> — <variant> — phase-N — Tasks

- [ ] <step 1>
- [ ] <step 2>
- [ ] Lint / typecheck / tests pass
- [ ] Self-review diff
```

### Ownership

- `phases.md` — the worker checks phases off and appends "Done" / "Rework round
  N" sections; never rewrite the researcher's design.
- `phase-N-plan.md` — append-only for the worker (add a `## Deviations` section
  if needed).
- `phase-N-task.md` — the worker mutates freely: tick items, add discovered
  sub-tasks, strike abandoned ones with a one-line reason.
