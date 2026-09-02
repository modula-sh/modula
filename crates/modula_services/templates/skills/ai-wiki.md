## Skill: AI Wiki

The workspace `wiki/` is an agent-maintained knowledge base of durable codebase
facts — architecture, conventions, quirks — **never** task-specific commentary.

- **Before working**, read `wiki/index.md` and any pages relevant to the
  projects/components you'll touch. Cite pages as `[[link]]` in specs.
- **When your work surfaces a durable fact** that contradicts, fills a gap in,
  or extends the wiki: **read `wiki/SCHEMA.md` first**, then update the relevant
  page(s), keep `wiki/index.md` in sync, and append an entry to `wiki/log.md`.
- Keep it durable: if a fact only matters to this task, it belongs in the spec
  or a thread comment, not the wiki.
