## Skill: Worktrees

Each task carries a `worktree` flag (default **true**).

- **Worktree mode** (`worktree: true` or unset): every project gets a dedicated
  git worktree + branch, so variants stay isolated.
- **Direct mode** (`worktree: false`): work directly on `base_branch`, no new
  branch. The researcher should have produced exactly one variant.

Branch name: `feature/<task-slug>-v<position>` (slug-based, never UUIDs) —
`<task-slug>` and `<position>` are the same ones in this run's spec folder
(`specs/<task-slug>/v<position>/`, named in your prompt). E.g. spec folder
`specs/mod-0001-some-new-adjustment/v1/` → branch
`feature/mod-0001-some-new-adjustment-v1`.

### Setting up (worker)

```bash
cd <project_path>
# Worktree mode:
git worktree add .worktrees/<branch> -b <branch> <base_branch>   # fresh
git worktree add .worktrees/<branch> <branch>                    # rework / reuse

# Direct mode:
git checkout <base_branch> && git pull --ff-only
git tag -f modula/<task-slug>-v<position>/start                 # fresh only; tree must be clean
```

`<project_path>` and `<base_branch>` come from `/config` `.projects`.

### Inspecting the diff (reviewer)

```bash
# Worktree mode:
cd <project_path>/.worktrees/<branch>
git log  <base_branch>..<branch>
git diff <base_branch>..<branch>

# Direct mode (the start tag marks the pre-work commit):
cd <project_path>
git log  modula/<task-slug>-v<position>/start..<base_branch>
git diff modula/<task-slug>-v<position>/start..<base_branch>
```

### Rules

- **Never push to a remote** (`git push`, `gh pr create`, …).
- Stay scoped to your own variant's worktree(s) / checkout; treat every other
  variant's worktree as read-only.
- Commit incrementally per each project's commit style.
