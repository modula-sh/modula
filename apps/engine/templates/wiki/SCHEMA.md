# Workspace AI Wiki — Schema

This wiki is an agent-maintained knowledge base for this workspace. It
covers everything an agent (or a human) needs to know about the projects
under management — architecture, build and run instructions, deployment,
operational lore, integration contracts, conventions, gotchas — anything
durable that would otherwise have to be re-discovered each time an agent
picks up a task. Agents (researcher, worker, code-reviewer) read it
before doing work and write to it when they discover new durable knowledge.
The user does not edit it directly.

The pattern follows
[karpathy's llm-wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f):
**raw sources** (the project repos, tasks, threads) are immutable; the
**wiki** is the LLM-owned synthesis layer; this **schema** tells the agent
how to use it.

Open this folder in Obsidian (or any wiki-link-aware reader) for graph view
and `[[wiki-link]]` navigation.

## Layout

```
wiki/
├── SCHEMA.md       ← this file
├── index.md        ← catalog — every page listed with a one-line summary
├── log.md          ← chronological — ## [YYYY-MM-DD] kind | title
├── general/        ← workspace-wide pages (cross-project, factory ops)
└── <project>/      ← one dir per `projects` row name
```

There is exactly **one** `index.md` and **one** `log.md`, at the wiki root.
The per-project subdirs are categories within the catalog, not nested wikis.

## What goes in the wiki

**Guiding principle:** the wiki captures knowledge that would otherwise have
to be *re-discovered* on every task. If a fact is already pinned in a
source an agent reads anyway (the agent's required reading, project rule
sheets, project README/docs), it's already in the agent's working context —
restating it in the wiki adds bloat and creates drift when the source
changes.

DO capture (durable codebase truths *not already authoritative elsewhere*):

- Architectural patterns and the reasoning behind them
- Module / package responsibilities and boundaries
- Recurring conventions (naming, error handling, testing style) — only
  where the project's own `CLAUDE.md` / docs are silent on them
- Quirks: gotchas, undocumented constraints, why X exists the way it does
- Cross-project contracts (e.g. "service A expects header X from service B")
- Contradictions discovered between docs and code

DO NOT capture (these belong elsewhere):

- Task-specific work decisions → `specs/<task-slug>/v<position>/phases.md`, `phase-N-plan.md`, `phase-N-task.md`
- Diff descriptions, PR notes → thread entries (variant- or task-scoped)
  via `POST /threads/...`
- Build/test logs → `logs/`
- Code itself (lives in the repos)
- Content authoritative in `<project>/CLAUDE.md` — that file is the
  project's committed rule sheet; the wiki **complements** CLAUDE.md
  (covers what it doesn't), it never restates it. If something becomes
  durable enough to pin, the right move is usually to add it to
  CLAUDE.md, not the wiki.
- Verbatim project documentation (`<project>/README.md`, `<project>/docs/`,
  in-repo wikis). Cite or link to those; don't mirror them. A wiki page
  may *interpret* or *connect* them, but shouldn't restate them.

The wiki is **about** the projects — how to reason about them, build them,
run them, deploy them, integrate them, and operate them — synthesized from
the code and from each project's own docs. It is neither a copy of the
code nor a duplicate of project-internal docs.

## Reading: when an agent picks up work

1. Read `index.md` first.
2. Drill into pages relevant to the projects/components in your assignment.
3. Use what you find. Cite pages with `[[page-name]]` in your output where
   you're explaining a decision that builds on existing knowledge.

## Writing: ingest a new finding

When your work surfaces a durable fact not yet in the wiki:

1. Create or update the relevant page(s) in `general/` or `<project>/`.
2. Cross-reference with Obsidian wiki-link syntax: `[[page-name]]` for
   pages with unique names, `[[<project>/page-name]]` when there's
   ambiguity. This populates the Obsidian graph view.
3. Update `index.md` — add a new row or revise an existing one-line summary.
4. Append to `log.md`:

   ```
   ## [YYYY-MM-DD] kind | title
   One or two lines of context.
   ```

   `kind` is one of: `ingest` (new page), `update` (revised page),
   `contradict` (resolved a contradiction with a prior page), `lint`
   (maintenance pass).

A single ingest typically touches several pages — entity pages, concept
pages, plus the index and log.

## Query: file durable answers back

When your investigation produces a *new* synthesis — a comparison between
two components, a discovered connection, an analysis that would help
future agents — file it back into the wiki as its own page rather than
letting it disappear into thread comments or chat history. This is the
gist's third Operation: *good answers compound into the knowledge base
just like ingested sources do.*

Use the same Ingest mechanics: create the page, cross-link it with
`[[other-page]]`, update `index.md`, append to `log.md` with
`kind: ingest`. The trigger is different (your own synthesis, not a new
source), but the writes are the same.

If the synthesis only restates existing pages without adding insight, skip
it — see the DO-NOT-capture list above. The bar is "would the next agent
benefit from finding this in `index.md`?"

## Page conventions

- **Filename**: kebab-case, no spaces. `auth-system.md`, `migrations.md`.
- **First line** is an H1 with the page title; this is what `[[name]]` links
  resolve to.
- **YAML frontmatter** is optional but useful for Obsidian Dataview:

   ```yaml
   ---
   tags: [architecture, project:<name>]
   updated: YYYY-MM-DD
   ---
   ```

- **Cross-reference liberally**. Pages should be small and well-linked.
  Multiple short pages > one giant page.
- **Be concise.** A reader should be able to skim a page in 30 seconds and
  know whether it's relevant.

### Resolving contradictions

When a finding contradicts or supersedes an existing page, the
contradiction must be visible to a future agent who lands on either page
— logging it in `log.md` alone is not enough (the next read won't grep
the log). Make it durable on the pages themselves:

- On the **new / corrected** page, add a `contradicts: [[old-page]]`
  field in frontmatter (or right under the H1 if no frontmatter), with a
  one-line note explaining what changed.
- On the **old** page, if it's still useful to keep around, add a
  `superseded_by: [[new-page]]` field so a reader landing there is
  immediately redirected. If the old page is wholly wrong and not worth
  keeping, delete it and let the link to it in the new page's
  `contradicts:` serve as the audit trail (the `log.md` entry retains
  the full record).
- Append a `## [YYYY-MM-DD] contradict | <title>` entry to `log.md`
  noting which page replaced which.

Frontmatter shape:

```yaml
---
tags: [architecture, project:<name>]
updated: YYYY-MM-DD
contradicts: [[old-page]]      # only when applicable
superseded_by: [[new-page]]    # only when applicable
---
```

## Lint (maintenance)

Periodically — usually triggered manually — an agent should health-check
the wiki:

- Contradictions between pages
- Stale claims that newer findings supersede
- Orphan pages with no inbound `[[links]]`
- Concepts mentioned but lacking their own page
- Missing cross-references
- Data gaps that could be filled by reading more of the codebase

A lint pass appends to `log.md` with `kind: lint`.
