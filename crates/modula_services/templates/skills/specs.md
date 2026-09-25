## Skill: Specs

A spec folder is the durable record of one variant's design and progress. It lives under the workspace root and is named by slug, never by UUID.

```text
specs/<task-slug>/v<position>/
    phases.md          # variant overview and phase checklist (source of truth)
    phase-1-plan.md    # scope, approach, and notes for phase 1
    phase-1-task.md    # actionable checklist for phase 1
    phase-2-plan.md
    phase-2-task.md
    ...
```

- `<task-slug>` is the task's external id and title, slugified: `mod-0001-some-new-adjustment`.
- `<position>` is the variant's 1-based position.
- Your prompt names the exact folder ("This run's spec folder is …"). Use that path. Don't construct it yourself.
- `phases.md` marks phases `- [ ]` (pending) or `- [x]` (done) and lists every project under "Projects touched".

### Write `phases.md`

```markdown
# <task title>

## Problem
What is broken, missing, or requested. Quote the task.

## Approach
The high-level approach this variant takes. One paragraph.

## Projects touched
Names must match `projects[].name` in `/config`. If none, say so and stop.

- project: <name>
  changes:
    - path/to/file: what changes

## Phases
- [ ] phase-1: <one-line summary>
- [ ] phase-2: <one-line summary>

## Risks & tradeoffs
- vs other variants: …
- known unknowns: …

## Test plan
- unit: …
- integration / manual: …
```

### Write `phase-N-plan.md`

```markdown
# <TASK> / <variant> / phase-N: <short title>

## Scope
What this phase delivers and what it defers.

## Approach
One paragraph.

## Notes
Implementation hints, edge cases, references.
```

### Write `phase-N-task.md`

```markdown
# <TASK> / <variant> / phase-N: Tasks

- [ ] <step 1>
- [ ] <step 2>
- [ ] Lint, typecheck, and tests pass
- [ ] Self-review diff
```

### Who edits what

| File | Worker may |
| --- | --- |
| `phases.md` | Tick phases and append "Done" or "Rework round N" sections. Never rewrite the researcher's design. |
| `phase-N-plan.md` | Append only. Add a `## Deviations` section if needed. |
| `phase-N-task.md` | Edit freely: tick items, add discovered sub-tasks, strike abandoned ones with a one-line reason. |
