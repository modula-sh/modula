## Skill: Engine CLI

All workspace state lives in the engine. Reach it through the `modula` CLI. The CLI detects the engine URL and the current workspace, so never pass them.

### Read state

Reads print formatted text. Read it directly; don't parse it as JSON.

```sh
modula task list                 # all tasks
modula task get <task-id>        # one task with its variants
modula config get                # pipeline keys, projects, providers, agents
modula comment list <task-id>    # a task's thread
```

### Write state

Writes take one JSON body argument.

```sh
modula task create    '{ … }'
modula task patch     <task-id>    '{"status":"<pipeline-key>"}'
modula variant create <task-id>    '{"count":2}'
modula variant patch  <variant-id> '{"status":"in_progress"}'
modula variant patch  <variant-id> '{"action":"accept"}'
modula comment create <task-id>    '{"author":"<you>","kind":"comment","content":"…"}'
```

### Rules

- Take `<task-id>` and `<variant-id>` from the **Inputs for this run** block in your prompt. The engine mints ids as UUIDs. Never invent one.
- Read valid pipeline keys from `modula config get`. Never hardcode status keys.
- A non-zero exit means the call failed. Stop and report the error.
- Every write emits an event, and the dispatcher routes it to the next agent. Never spawn agents yourself.
