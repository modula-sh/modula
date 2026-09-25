## Skill: AI Wiki

The workspace `wiki/` is an agent-maintained knowledge base of durable codebase facts: architecture, conventions, quirks. It **never** holds task-specific commentary.

- **Before you work**, read `wiki/index.md` and the pages for the projects and components you'll touch. Cite pages as `[[link]]` in specs.
- **When you find a durable fact** that contradicts, fills a gap in, or extends the wiki, **read `wiki/SCHEMA.md` first**. Then update the relevant pages, keep `wiki/index.md` in sync, and append an entry to `wiki/log.md`.
- **Keep it durable.** A fact that matters only to this task belongs in the spec or a thread comment, not the wiki.

### Writing style

Agents read wiki pages to act. Write terse, factual, instructional pages.

**Page shape**

- H1 title in sentence case, then a 1–2 sentence intro stating what the page covers and when it's relevant.
- Order sections: what it is → reference → how to → when it breaks. Skip any that don't apply.
- Name sections by task: "Run the migrations", not "Migrations". Don't restate the heading in the first sentence. Add the next fact.
- Keep one topic per page. Split a page when it covers two things, and link the halves.

**Sentences**

- Lead with the fact. Cut the lead-in ("In order to understand…").
- Imperative and second person: "Run `make db`", not "We run `make db`" or "`make db` should be run".
- One idea per sentence. Split a rule from its exception.
- Cut filler: `simply`, `just`, `easily`, `powerful`, `robust`, `note that`, `it's important to`.
- No em dashes. Use a full stop, colon, or comma.

**Formatting**

- Numbered lists for ordered steps, bullets for everything else.
- Tables for anything that enumerates: flags, env vars, fields, states, exit codes.
- Code, paths, commands, and identifiers in backticks. Placeholders in ALL-CAPS: `modula task get TASK_ID`.
- Fenced code blocks carry a language tag.
- Link text describes the target. Link once, where the topic comes up.
- No callouts or alert boxes unless ignoring the point breaks something.

**Accuracy**

- Write only what you verified in code or by running it. If something is unknown, leave it out or mark it `unverified`.
- Show real commands and real output. Invented output goes stale unnoticed.
- Document the failure: error text, cause, fix. A happy-path-only page is half a page.
