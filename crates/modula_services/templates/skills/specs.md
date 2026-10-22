## Skill: Specs

A variant's plan is the durable record of its design and progress: **one JSON document per variant**, stored by the engine. Read and write it only through `modula plan`. Never write plan files. Every write is validated, so a bad document fails with the reason, and publishes `plan.update`.

```sh
modula plan get   <variant-id>          # the document as JSON
modula plan get   <variant-id> --md     # readable view: spec sections plus a checklist per phase
modula plan set   <variant-id>          # whole-plan write, body on stdin (or --file PATH)
modula plan patch <variant-id> '<body>' # one targeted edit
```

`modula variant get` and `modula task get` print a one-line progress summary per variant.

If `modula plan get` errors with `has no plan`, the variant predates plans: use the markdown files in your spec folder (`phases.md`, `phase-N-plan.md`, `phase-N-task.md`) as before.

### Plan document

```json
{
  "version": 1,
  "title": "<task external id> <task title>",
  "problem": "What is broken, missing, or requested. Quote the task.",
  "approach": "The high-level approach this variant takes. One paragraph.",
  "projects": [
    { "name": "<project>", "changes": [{ "path": "path/to/file", "change": "what changes" }] }
  ],
  "phases": [
    {
      "title": "<short title>",
      "scope": "What this phase delivers and what it defers.",
      "approach": "One paragraph.",
      "notes": "Implementation hints, edge cases, references.",
      "tasks": [
        { "title": "<step 1>" },
        { "title": "Lint, typecheck, and tests pass" },
        { "title": "Self-review diff" }
      ]
    }
  ],
  "risks": "- vs other variants: …\n- known unknowns: …",
  "test_plan": "- unit: …\n- integration / manual: …"
}
```

- Prose fields are markdown strings. Keep large code blocks out.
- `projects[].name` must match `projects[].name` in `modula config get`. The Worker sets up exactly these projects.
- Every phase needs a title and at least one task.
- The engine assigns ids by position: phases `1..N`, tasks `<phase>.1..<phase>.M` (`2.3` is phase 2's third task). Omit `id`; any supplied one is overwritten.
- Tasks start `pending`. Omit `status` and `note`.
- Phase status and percent complete are derived from task statuses, never stored. A phase is done when every task is `done` or `skipped`.
- Unknown keys are rejected. Optional fields: `notes`, plus the Worker-owned `deviations` and `summaries`.

### Write the plan (Researcher)

Pass the JSON through a quoted heredoc, so the shell leaves quotes and backticks alone:

```sh
modula plan set <variant-id> <<'EOF'
{ "version": 1, … }
EOF
```

`set` replaces the whole document. Fix the reported error and re-run it until it succeeds.

### Advance the plan (Worker)

| Body | Edit |
| --- | --- |
| `{"task":"1.2","status":"in_progress"}` | Start a task. |
| `{"task":"1.2","status":"done"}` | Finish the task you started. |
| `{"task":"1.2","status":"skipped","note":"why"}` | Abandon a task. The note is required. |
| `{"task":"1.2","status":"pending"}` | Re-open a task. |
| `{"phase":"2","add_task":"Title"}` | Append a discovered task to a phase. |
| `{"add_phase":{"title":"…","scope":"…","approach":"…","notes":"…","tasks":["…"]}}` | Append a phase, for example a rework phase. |
| `{"phase":"1","deviations":"…"}` | Record how a phase departed from its plan. |
| `{"summary":{"kind":"done","content":"…"}}` | Append the Done summary. |
| `{"summary":{"kind":"rework","round":N,"content":"…"}}` | Append the Rework round N summary. |

- Mark a task `in_progress` when you start it and `done` when you finish it, one at a time. Never batch-complete tasks afterwards.
- At most one task is `in_progress`. `done` is accepted only from `in_progress`.
- `add_task` and `add_phase` print the new id.

### Who edits what

| Part | Researcher | Worker |
| --- | --- | --- |
| `title`, `problem`, `approach`, `projects`, `risks`, `test_plan` | Writes (`plan set`). | Read only. |
| Phase `title`, `scope`, `approach`, `notes` | Writes. | Read only. Record departures in `deviations`. |
| Phase `deviations` | — | Append only. |
| Tasks | Writes the list. | Change status, append discovered tasks, skip abandoned ones with a one-line note. |
| Phases | Writes the list. | Append a rework phase. |
| `summaries` | — | Append Done and Rework round N summaries. |

The Worker never edits or removes the Researcher's structure or prose. `plan patch` offers no operation that could.

### Spec folder

Your prompt names a spec folder (`specs/<task-slug>/v<position>`). It no longer stores the plan. Its name gives the branch (see the Worktrees skill), and it holds the markdown specs of variants that predate plans.
