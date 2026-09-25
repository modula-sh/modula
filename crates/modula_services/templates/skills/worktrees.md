## Skill: Worktrees

Each task carries a `worktree` flag. It defaults to **true**.

| Mode | When | Behavior |
| --- | --- | --- |
| Worktree | `worktree: true` or unset | Each project gets its own git worktree and branch, so variants stay isolated. |
| Direct | `worktree: false` | Work on `base_branch` with no new branch. The researcher should have produced exactly one variant. |

Name the branch `feature/<task-slug>-v<position>`. Use slugs, never UUIDs. `<task-slug>` and `<position>` match your spec folder (`specs/<task-slug>/v<position>/`, named in your prompt). Spec folder `specs/mod-0001-some-new-adjustment/v1/` gives branch `feature/mod-0001-some-new-adjustment-v1`.

`<project_path>` and `<base_branch>` come from `.projects` in `/config`.

### Set up (worker)

```sh
cd <project_path>

# Worktree mode:
git worktree add .worktrees/<branch> -b <branch> <base_branch>   # fresh
git worktree add .worktrees/<branch> <branch>                    # rework or reuse

# Direct mode:
git checkout <base_branch> && git pull --ff-only
git tag -f modula/<task-slug>-v<position>/start                 # fresh only; tree must be clean
```

### Inspect the diff (reviewer)

```sh
# Worktree mode:
cd <project_path>/.worktrees/<branch>
git log  <base_branch>..<branch>
git diff <base_branch>..<branch>

# Direct mode. The start tag marks the commit before work began.
cd <project_path>
git log  modula/<task-slug>-v<position>/start..<base_branch>
git diff modula/<task-slug>-v<position>/start..<base_branch>
```

### Rules

- **Never push to a remote** (`git push`, `gh pr create`, …).
- Work only in your own variant's worktrees or checkout. Treat every other variant's worktree as read-only.
- Commit incrementally, following each project's commit style.
