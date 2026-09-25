## Skill: Tasks, Variants & Comments

### Read tasks and variants

```sh
modula task get <task-id>        # one task with its variants
modula variant get <variant-id>  # one variant (status, position) and its owning task
modula comment list <task-id>    # the task thread, then each variant's thread
```

A task carries human-owned fields (`approved`, `max_variants`, `worktree`) and a `variants[]` block. Each variant's `status` drives the pipeline. Read the printed `status:`, `approved:`, and per-variant lines directly.

### Register variants (researcher only)

```sh
modula variant create <task-id> '{"count":2}'
```

The engine mints the UUIDs and prints each created variant's id and position. A new variant has **no status** and is not workable until promoted.

### Set a variant's status

```sh
# Set a status directly.
modula variant patch <variant-id> '{"status":"in_progress"}'

# Code-reviewer applies a verdict through an action, not a raw status.
modula variant patch <variant-id> '{"action":"accept"}'
modula variant patch <variant-id> '{"action":"rework"}'
```

`variant patch` takes only the variant id. The CLI resolves the owning task.

### Post a comment

Threads are **append-only**. Every entry has an `author`, a `kind`, and `content`. Variant-scoped entries add `variant` and `round`. Task-scoped entries omit `variant`.

```sh
modula comment create <task-id> '{"author":"<you>","kind":"comment","variant":"<variant-id>","round":N,"content":"…"}'
```

| `kind` | Use |
| --- | --- |
| `comment` | An observation or note. |
| `question` | A question for the human. Code-reviewer must post at least one. |
| `verdict` | A decision. Set `verdict` to a value your role allows: `ACCEPT` or `REQUEST_CHANGES` for code-reviewer, `APPROVE` or `KICK_BACK` for reviewer. `KICK_BACK` requires `affected_variants`. |
| `rework` | The worker's summary of what a rework round fixed. |

#### Writing style

Humans skim threads. Write each entry to be read in seconds.

- **Lead with the point.** The first line is the finding, question, or decision. No preamble ("I reviewed…", "After looking at…"), no closing recap.
- **One point per entry.** Two unrelated findings are two entries.
- **Bullets over paragraphs.** Numbered list only for ordered steps. Keep formatting flat: no headings, tables, or nested lists. Bold only the single fact the reader must not miss.
- **Short, direct sentences.** Imperative and active voice: "Rename `foo`", not "It might be worth considering renaming `foo`".
- **Cut filler.** Drop `simply`, `just`, `clearly`, `robust`, `note that`, `it's important to`, and hedges like `I think` or `it seems`.
- **Be concrete.** Cite `file:line` and quote only the lines that matter in a fenced block. Put code, paths, and commands in backticks. Give the fix, not only the problem.
- **State only what you verified.** Don't guess at causes or behavior. If you didn't check something, say so in one line.
- **Add new information only.** Don't restate the task, the diff, or earlier entries. Reference them instead.
- **Make questions answerable.** Ask one question and offer the options: "Keep the old endpoint? (a) yes, deprecate later (b) no, remove now".
- **No em dashes.** Use a full stop, colon, or comma.

Before:

> After carefully reviewing the changes in this variant, I noticed that there might potentially be an issue with how errors are handled in the sync path. It seems like the error is simply being swallowed, which could make debugging harder down the line, so it would probably be good to consider logging it.

After:

> `sync.rs:88` swallows the error from `fetch()`.
> - Log it with `tracing::warn!` or return it.
