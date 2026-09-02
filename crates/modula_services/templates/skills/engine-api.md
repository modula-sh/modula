## Skill: Engine CLI

All workspace state lives in the engine and is reached through the `modula` CLI.
The engine URL and the current workspace are detected automatically — you never
pass them.

Reads print formatted text (read it directly; do not parse as JSON):

    modula task list                 # all tasks
    modula task get <task-id>        # one task with its variants
    modula config get                # pipeline keys, projects, providers, agents
    modula comment list <task-id>    # a task's thread

Writes take a single JSON body argument:

    modula task create   '{ … }'
    modula task patch    <task-id>    '{"status":"<pipeline-key>"}'
    modula variant create <task-id>   '{"count":2}'
    modula variant patch  <variant-id> '{"status":"in_progress"}'
    modula variant patch  <variant-id> '{"action":"accept"}'
    modula comment create <task-id>   '{"author":"<you>","kind":"comment","content":"…"}'

Conventions:
- Fill `<task-id>` / `<variant-id>` from the **Inputs for this run** block in your
  prompt. Ids are UUIDs minted by the engine — never invent them.
- Read valid pipeline keys from `modula config get`; never hardcode status keys.
- A non-zero exit means the call failed — stop and surface the error.
- Every write emits an event the dispatcher routes to the next agent; you never
  spawn other agents yourself.
