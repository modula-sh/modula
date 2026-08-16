import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { syntaxTree } from "@codemirror/language";
import { Facet, type Range } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  keymap,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from "@codemirror/view";
import { Vim, vim } from "@replit/codemirror-vim";
import { useEffect, useRef } from "react";
import { openUrl } from "./openUrl";

// Obsidian-style live preview. Add a syntax = entry in STYLE_BY_NODE + (if markers) MARKER_NODES.

// Facet routing vim `:w` / `:wq` to the host's save handler.
const saveCallback = Facet.define<() => void, () => void>({
  combine: (values) => values[0] ?? (() => {}),
});
const runSave = (cm: { cm6: EditorView }) => cm.cm6.state.facet(saveCallback)();
Vim.defineEx("write", "w", runSave);
Vim.defineEx("wq", "wq", runSave);

const HIDE = Decoration.replace({});

class BulletWidget extends WidgetType {
  toDOM() {
    const el = document.createElement("span");
    el.className = "cm-md-bullet";
    el.textContent = "•";
    return el;
  }
  eq() {
    return true;
  }
}
const BULLET = Decoration.replace({ widget: new BulletWidget() });

class CheckboxWidget extends WidgetType {
  constructor(readonly checked: boolean) {
    super();
  }
  toDOM(view: EditorView) {
    const el = document.createElement("input");
    el.type = "checkbox";
    el.checked = this.checked;
    el.className = "cm-md-taskbox";
    el.addEventListener("mousedown", (e) => {
      e.preventDefault();
      e.stopPropagation();
      const pos = view.posAtDOM(el);
      const cur = view.state.doc.sliceString(pos, pos + 3);
      const next = /[xX]/.test(cur) ? "[ ]" : "[x]";
      view.dispatch({ changes: { from: pos, to: pos + 3, insert: next } });
    });
    return el;
  }
  eq(other: CheckboxWidget) {
    return this.checked === other.checked;
  }
}
function checkbox(checked: boolean): Decoration {
  return Decoration.replace({ widget: new CheckboxWidget(checked) });
}

type ParsedTable = {
  rows: string[][];
  aligns: Array<"left" | "center" | "right">;
};

function parseTable(text: string): ParsedTable {
  const lines = text.split("\n").filter((l) => l.trim().length > 0);
  if (lines.length < 2) return { rows: [], aligns: [] };
  const split = (line: string) => {
    let s = line.trim();
    if (s.startsWith("|")) s = s.slice(1);
    if (s.endsWith("|")) s = s.slice(0, -1);
    return s.split("|").map((c) => c.trim());
  };
  const aligns: ParsedTable["aligns"] = split(lines[1]).map((c) => {
    const l = c.startsWith(":");
    const r = c.endsWith(":");
    if (l && r) return "center";
    if (r) return "right";
    return "left";
  });
  const rows = [split(lines[0])];
  for (let i = 2; i < lines.length; i++) rows.push(split(lines[i]));
  return { rows, aligns };
}

function emitTableMarkdown(p: ParsedTable): string {
  // Always outer-pipe form; bare form loses empty trailing cells on round-trip.
  const colCount = Math.max(p.rows[0]?.length ?? 0, ...p.rows.map((r) => r.length));
  const widths = Array(colCount).fill(3);
  for (const row of p.rows) {
    row.forEach((c, i) => {
      widths[i] = Math.max(widths[i], c.length);
    });
  }
  const pad = (c: string, i: number) => ` ${c.padEnd(widths[i])} `;
  const renderRow = (row: string[]) => {
    const cells = Array(colCount)
      .fill("")
      .map((_, i) => pad(row[i] ?? "", i));
    return `|${cells.join("|")}|`;
  };
  const renderDelim = () => {
    const cells = Array(colCount)
      .fill("")
      .map((_, i) => {
        const w = widths[i];
        const a = p.aligns[i] ?? "left";
        if (a === "center") return `:${"-".repeat(w)}:`;
        if (a === "right") return `${"-".repeat(w + 1)}:`;
        return "-".repeat(w + 2);
      });
    return `|${cells.join("|")}|`;
  };
  const lines: string[] = [renderRow(p.rows[0] ?? []), renderDelim()];
  for (let i = 1; i < p.rows.length; i++) lines.push(renderRow(p.rows[i]));
  return lines.join("\n");
}

function findTables(view: EditorView): Array<{ from: number; to: number }> {
  const tables: Array<{ from: number; to: number }> = [];
  syntaxTree(view.state).iterate({
    enter(node) {
      if (node.type.name === "Table") {
        tables.push({ from: node.from, to: node.to });
        return false;
      }
    },
  });
  return tables;
}

type TableEditState = { isEditing: boolean };

// Resolve current Table at/near `hint` (closures over from/to go stale post-dispatch).
function mutateTable(view: EditorView, hint: number, mutate: (p: ParsedTable) => void): void {
  const tree = syntaxTree(view.state);
  type SN = ReturnType<typeof tree.resolveInner>;
  let n: SN | null = tree.resolveInner(hint);
  while (n && n.type.name !== "Table") n = n.parent;
  if (!n) {
    // Hint isn't inside a table anymore — search forward for the next one.
    let found: SN | null = null;
    tree.iterate({
      from: Math.max(0, hint),
      enter(node) {
        if (found) return false;
        if (node.type.name === "Table") {
          found = node.node;
          return false;
        }
      },
    });
    n = found;
  }
  if (!n) return;
  const current = parseTable(view.state.doc.sliceString(n.from, n.to));
  if (current.rows.length === 0) return;
  mutate(current);
  view.dispatch({
    changes: { from: n.from, to: n.to, insert: emitTableMarkdown(current) },
  });
}

function renderTableElement(
  parsed: ParsedTable,
  view: EditorView,
  state: TableEditState,
  hint: number,
): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "cm-md-table-wrap";
  const frame = document.createElement("div");
  frame.className = "cm-md-table-frame";
  wrap.appendChild(frame);
  const lineH = view.defaultLineHeight;

  const buildRow = (cells: string[], rowIdx: number, isHeader: boolean) => {
    const tr = document.createElement("tr");
    tr.style.height = `${lineH}px`;
    for (let j = 0; j < cells.length; j++) {
      const cell = document.createElement(isHeader ? "th" : "td");
      cell.textContent = cells[j];
      cell.contentEditable = "true";
      cell.spellcheck = false;
      const a = parsed.aligns[j];
      if (a) cell.style.textAlign = a;
      cell.addEventListener("focus", () => {
        state.isEditing = true;
      });
      cell.addEventListener("blur", () => {
        state.isEditing = false;
        const v = (cell.textContent ?? "").replace(/\n/g, " ").trim();
        mutateTable(view, hint, (p) => {
          if (p.rows[rowIdx]?.[j] !== undefined && p.rows[rowIdx][j] !== v) {
            p.rows[rowIdx][j] = v;
          }
        });
      });
      cell.addEventListener("keydown", (e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          (e.target as HTMLElement).blur();
        } else if (e.key === "Escape") {
          cell.textContent = parsed.rows[rowIdx]?.[j] ?? "";
          (e.target as HTMLElement).blur();
        }
      });
      tr.appendChild(cell);
    }
    return tr;
  };

  const table = document.createElement("table");
  table.className = "cm-md-table";
  const thead = document.createElement("thead");
  thead.appendChild(buildRow(parsed.rows[0] ?? [], 0, true));
  table.appendChild(thead);
  const tbody = document.createElement("tbody");
  for (let i = 1; i < parsed.rows.length; i++) {
    tbody.appendChild(buildRow(parsed.rows[i], i, false));
  }
  table.appendChild(tbody);
  frame.appendChild(table);

  const addBtn = (cls: string, title: string, mut: (p: ParsedTable) => void) => {
    const b = document.createElement("button");
    b.type = "button";
    b.className = `cm-md-table-add ${cls}`;
    b.title = title;
    b.textContent = "+";
    b.addEventListener("mousedown", (e) => {
      e.preventDefault();
      e.stopPropagation();
      mutateTable(view, hint, mut);
    });
    frame.appendChild(b);
  };
  addBtn("cm-md-table-add-col", "Add column", (p) => {
    p.rows.forEach((r) => r.push(""));
    p.aligns.push("left");
  });
  addBtn("cm-md-table-add-row", "Add row", (p) => {
    const cols = p.rows[0]?.length ?? 1;
    p.rows.push(Array(cols).fill(""));
  });

  return wrap;
}

const tableOverlay = ViewPlugin.fromClass(
  class {
    overlay: HTMLElement;
    state: TableEditState = { isEditing: false };
    constructor(view: EditorView) {
      this.overlay = document.createElement("div");
      this.overlay.className = "cm-md-table-overlay";
      view.scrollDOM.appendChild(this.overlay);
      this.render(view);
    }
    update(u: ViewUpdate) {
      if (
        u.docChanged ||
        u.viewportChanged ||
        u.geometryChanged ||
        u.selectionSet ||
        u.focusChanged
      ) {
        this.render(u.view);
      }
    }
    destroy() {
      this.overlay.remove();
    }
    render(view: EditorView) {
      if (this.state.isEditing) return;
      this.overlay.replaceChildren();
      const tables = findTables(view);
      const focused = focusedLines(view);
      const doc = view.state.doc;
      // Offset because the overlay sits in scrollDOM, not cm-content.
      const cmTopInScroll =
        view.contentDOM.offsetTop + parseFloat(getComputedStyle(view.contentDOM).paddingTop || "0");
      for (const t of tables) {
        const startL = doc.lineAt(t.from).number;
        const endL = doc.lineAt(Math.max(t.from, t.to - 1)).number;
        let anyFocused = false;
        for (let l = startL; l <= endL; l++) {
          if (focused.has(l)) {
            anyFocused = true;
            break;
          }
        }
        if (anyFocused) continue;
        const parsed = parseTable(doc.sliceString(t.from, t.to));
        if (parsed.rows.length === 0) continue;
        const top = view.lineBlockAt(t.from).top + cmTopInScroll;
        const el = renderTableElement(parsed, view, this.state, t.from);
        el.style.top = `${top}px`;
        this.overlay.appendChild(el);
      }
    }
  },
);

class ImageWidget extends WidgetType {
  constructor(
    readonly url: string,
    readonly alt: string,
  ) {
    super();
  }
  toDOM() {
    const el = document.createElement("img");
    el.src = this.url;
    el.alt = this.alt;
    el.className = "cm-md-image-rendered";
    return el;
  }
  eq(other: ImageWidget) {
    return this.url === other.url && this.alt === other.alt;
  }
}
function imageReplace(url: string, alt: string): Decoration {
  return Decoration.replace({ widget: new ImageWidget(url, alt) });
}

const STYLE_BY_NODE: Record<string, string> = {
  StrongEmphasis: "cm-md-bold",
  Emphasis: "cm-md-italic",
  InlineCode: "cm-md-code",
  Strikethrough: "cm-md-strike",
  ATXHeading1: "cm-md-h1",
  ATXHeading2: "cm-md-h2",
  ATXHeading3: "cm-md-h3",
  ATXHeading4: "cm-md-h4",
  ATXHeading5: "cm-md-h5",
  ATXHeading6: "cm-md-h6",
  HorizontalRule: "cm-md-hr",
  Link: "cm-md-link",
  Autolink: "cm-md-link",
  Image: "cm-md-image",
  Comment: "cm-md-comment",
  CommentBlock: "cm-md-comment",
};

const MARKER_NODES: ReadonlySet<string> = new Set([
  "HeaderMark",
  "EmphasisMark",
  "StrikethroughMark",
  "LinkMark",
  "QuoteMark",
  "CodeMark", // inline only; FencedCode handles its own
  // URL handled specially below: hide inside Link, keep visible inside Autolink.
]);

const LINE_STYLE_BY_NODE: Record<string, string> = {
  Blockquote: "cm-md-quote-line",
};

const markCache: Record<string, Decoration> = {};
function styledMark(cls: string): Decoration {
  return (markCache[cls] ??= Decoration.mark({ class: cls }));
}

const lineCache: Record<string, Decoration> = {};
function styledLine(cls: string): Decoration {
  return (lineCache[cls] ??= Decoration.line({ class: cls }));
}

// `---\n…\n---` at top of file. Lezer doesn't parse it natively; we do.
function detectFrontmatter(
  doc: EditorView["state"]["doc"],
): { startLine: number; endLine: number } | null {
  if (doc.lines < 2) return null;
  if (doc.line(1).text.trim() !== "---") return null;
  for (let i = 2; i <= doc.lines; i++) {
    if (doc.line(i).text.trim() === "---") return { startLine: 1, endLine: i };
  }
  return null;
}

type FmEntry = {
  key: string;
  value: { kind: "array"; items: string[] } | { kind: "plain"; value: string };
};

function parseFrontmatter(
  doc: EditorView["state"]["doc"],
  fm: { startLine: number; endLine: number },
): FmEntry[] {
  const entries: FmEntry[] = [];
  for (let l = fm.startLine + 1; l < fm.endLine; l++) {
    const line = doc.line(l).text;
    const colon = line.indexOf(":");
    if (colon < 0) continue;
    const key = line.slice(0, colon).trim();
    const raw = line.slice(colon + 1).trim();
    if (raw.startsWith("[") && raw.endsWith("]")) {
      const items = raw
        .slice(1, -1)
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);
      entries.push({ key, value: { kind: "array", items } });
    } else {
      entries.push({ key, value: { kind: "plain", value: raw } });
    }
  }
  return entries;
}

function renderFrontmatterElement(entries: FmEntry[]): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "cm-md-fm-wrap";
  for (const entry of entries) {
    const row = document.createElement("div");
    row.className = "cm-md-fm-row";
    const keyEl = document.createElement("span");
    keyEl.className = "cm-md-fm-key";
    keyEl.textContent = entry.key;
    row.appendChild(keyEl);
    const valEl = document.createElement("span");
    valEl.className = "cm-md-fm-value";
    if (entry.value.kind === "array") {
      for (const item of entry.value.items) {
        const pill = document.createElement("span");
        pill.className = "cm-md-fm-pill";
        pill.textContent = item;
        valEl.appendChild(pill);
      }
    } else {
      valEl.textContent = entry.value.value;
    }
    row.appendChild(valEl);
    wrap.appendChild(row);
  }
  return wrap;
}

const frontmatterOverlay = ViewPlugin.fromClass(
  class {
    overlay: HTMLElement;
    constructor(view: EditorView) {
      this.overlay = document.createElement("div");
      this.overlay.className = "cm-md-fm-overlay";
      view.scrollDOM.appendChild(this.overlay);
      this.render(view);
    }
    update(u: ViewUpdate) {
      if (
        u.docChanged ||
        u.viewportChanged ||
        u.geometryChanged ||
        u.selectionSet ||
        u.focusChanged
      ) {
        this.render(u.view);
      }
    }
    destroy() {
      this.overlay.remove();
    }
    render(view: EditorView) {
      this.overlay.replaceChildren();
      const fm = detectFrontmatter(view.state.doc);
      if (!fm) return;
      const focused = focusedLines(view);
      for (let l = fm.startLine; l <= fm.endLine; l++) {
        if (focused.has(l)) return;
      }
      const entries = parseFrontmatter(view.state.doc, fm);
      if (entries.length === 0) return;
      const cmTopInScroll =
        view.contentDOM.offsetTop + parseFloat(getComputedStyle(view.contentDOM).paddingTop || "0");
      const top = view.lineBlockAt(view.state.doc.line(fm.startLine).from).top + cmTopInScroll;
      const totalLines = fm.endLine - fm.startLine + 1;
      const height = totalLines * view.defaultLineHeight;
      const el = renderFrontmatterElement(entries);
      el.style.top = `${top}px`;
      el.style.height = `${height}px`;
      this.overlay.appendChild(el);
    }
  },
);

function focusedLines(view: EditorView): Set<number> {
  const lines = new Set<number>();
  // Default selection at pos 0 is meaningless when editor isn't focused.
  if (!view.hasFocus) return lines;
  for (const r of view.state.selection.ranges) {
    const s = view.state.doc.lineAt(r.from).number;
    const e = view.state.doc.lineAt(r.to).number;
    for (let i = s; i <= e; i++) lines.add(i);
  }
  return lines;
}

function buildDecorations(view: EditorView): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  const focused = focusedLines(view);
  const doc = view.state.doc;

  // YAML frontmatter: hide all source lines when no fm line is focused; the
  // `frontmatterOverlay` plugin renders the parsed properties as pills/rows.
  const fm = detectFrontmatter(doc);
  if (fm) {
    let anyFmFocused = false;
    for (let l = fm.startLine; l <= fm.endLine; l++) {
      if (focused.has(l)) {
        anyFmFocused = true;
        break;
      }
    }
    if (!anyFmFocused) {
      for (let l = fm.startLine; l <= fm.endLine; l++) {
        ranges.push(styledLine("cm-md-fm-hidden").range(doc.line(l).from));
      }
    }
  }

  syntaxTree(view.state).iterate({
    enter(node) {
      const name = node.type.name;
      const { from, to } = node;
      if (from === to) return;

      // Code blocks: line-style every line, hide fence lines as single ranges.
      if (name === "FencedCode" || name === "CodeBlock") {
        const firstLine = doc.lineAt(from);
        const lastLine = doc.lineAt(Math.max(from, to - 1));
        // Language tag → pill on the first line (only when unfocused so it
        // doesn't double up with the raw ```lang text).
        let lang: string | null = null;
        if (name === "FencedCode") {
          let child = node.node.firstChild;
          while (child) {
            if (child.type.name === "CodeInfo") {
              lang = doc.sliceString(child.from, child.to).trim();
              break;
            }
            child = child.nextSibling;
          }
        }
        const firstIsFocused = focused.has(firstLine.number);
        const firstLineDeco =
          lang && !firstIsFocused
            ? Decoration.line({
                class: "cm-md-code-line",
                attributes: { "data-lang": lang },
              })
            : styledLine("cm-md-code-line");
        ranges.push(firstLineDeco.range(firstLine.from));
        for (let n = firstLine.number + 1; n <= lastLine.number; n++) {
          ranges.push(styledLine("cm-md-code-line").range(doc.line(n).from));
        }
        if (name === "FencedCode") {
          if (firstLine.to > firstLine.from && !firstIsFocused) {
            ranges.push(HIDE.range(firstLine.from, firstLine.to));
          }
          if (
            lastLine.number !== firstLine.number &&
            lastLine.to > lastLine.from &&
            !focused.has(lastLine.number)
          ) {
            ranges.push(HIDE.range(lastLine.from, lastLine.to));
          }
        }
        return false;
      }

      const styleClass = STYLE_BY_NODE[name];
      if (styleClass) ranges.push(styledMark(styleClass).range(from, to));
      const lineClass = LINE_STYLE_BY_NODE[name];
      if (lineClass) {
        let lineNo = doc.lineAt(from).number;
        const lastLine = doc.lineAt(Math.max(from, to - 1)).number;
        for (; lineNo <= lastLine; lineNo++) {
          ranges.push(styledLine(lineClass).range(doc.line(lineNo).from));
        }
      }
      if (MARKER_NODES.has(name) && !focused.has(doc.lineAt(from).number)) {
        // Eat the space after `#` so headings don't render with a leading gap.
        const hideEnd = name === "HeaderMark" && doc.sliceString(to, to + 1) === " " ? to + 1 : to;
        ranges.push(HIDE.range(from, hideEnd));
      }

      // URL: hide inside Link, link-style for bare GFM autolinks, plain inside <>.
      if (name === "URL") {
        const parent = node.node.parent?.type.name;
        if (parent === "Link") {
          if (!focused.has(doc.lineAt(from).number)) {
            ranges.push(HIDE.range(from, to));
          }
        } else if (parent !== "Autolink") {
          ranges.push(styledMark("cm-md-link").range(from, to));
        }
      }

      // Tables: hide raw markdown via opacity:0 line decoration when no row
      // is focused. The actual table is rendered by `tableOverlay` plugin
      // (absolute-positioned overlay outside CM6's measurement flow).
      if (name === "Table") {
        const startL = doc.lineAt(from).number;
        const endL = doc.lineAt(Math.max(from, to - 1)).number;
        let anyFocused = false;
        for (let l = startL; l <= endL; l++) {
          if (focused.has(l)) {
            anyFocused = true;
            break;
          }
        }
        if (!anyFocused) {
          for (let l = startL; l <= endL; l++) {
            ranges.push(styledLine("cm-md-table-line-hidden").range(doc.line(l).from));
          }
        }
        return false;
      }

      // Image `![alt](url)` → <img> widget when line not focused.
      if (name === "Image" && !focused.has(doc.lineAt(from).number)) {
        let url = "";
        let c = node.node.firstChild;
        while (c) {
          if (c.type.name === "URL") {
            url = doc.sliceString(c.from, c.to).trim().split(/\s+/)[0];
            break;
          }
          c = c.nextSibling;
        }
        if (url) {
          const altMatch = doc.sliceString(from, to).match(/^!\[([^\]]*)\]/);
          const alt = altMatch ? altMatch[1] : "";
          ranges.push(imageReplace(url, alt).range(from, to));
          return false;
        }
      }

      // GFM task marker `[ ]` / `[x]` → checkbox widget.
      if (name === "TaskMarker") {
        const checked = /[xX]/.test(doc.sliceString(from, to));
        ranges.push(checkbox(checked).range(from, to));
        return;
      }

      // List markers: replace bullets with `•` widget, fade numbered ones,
      // hide the dash entirely for task lines so only the checkbox shows.
      if (name === "ListMark") {
        const grand = node.node.parent?.parent;
        if (grand?.type.name === "BulletList") {
          const ahead = doc.sliceString(to, to + 5);
          if (/^\s\[[xX ]\]/.test(ahead)) {
            ranges.push(HIDE.range(from, to + 1));
          } else if (!focused.has(doc.lineAt(from).number)) {
            ranges.push(BULLET.range(from, to));
          }
        } else if (grand?.type.name === "OrderedList") {
          ranges.push(styledMark("cm-md-list-num").range(from, to));
        }
      }
    },
  });
  return Decoration.set(ranges, true);
}

const livePreview = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildDecorations(view);
    }
    update(u: ViewUpdate) {
      if (u.docChanged || u.viewportChanged || u.selectionSet || u.focusChanged) {
        this.decorations = buildDecorations(u.view);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

// Preload all <img> URLs in the doc so they're cached by the time the widget
// scrolls into view. Browser-level cache; no DOM nodes kept around.
const preloadedImages = new Set<string>();
function preloadDocImages(view: EditorView) {
  const doc = view.state.doc;
  syntaxTree(view.state).iterate({
    enter(node) {
      if (node.type.name !== "Image") return;
      let child = node.node.firstChild;
      while (child) {
        if (child.type.name === "URL") {
          const url = doc.sliceString(child.from, child.to).trim().split(/\s+/)[0];
          if (url && !preloadedImages.has(url)) {
            preloadedImages.add(url);
            const img = new Image();
            img.src = url;
          }
          break;
        }
        child = child.nextSibling;
      }
    },
  });
}
const imagePreloader = ViewPlugin.fromClass(
  class {
    constructor(view: EditorView) {
      preloadDocImages(view);
    }
    update(u: ViewUpdate) {
      if (u.docChanged) preloadDocImages(u.view);
    }
  },
);

// Click on rendered link text → open URL in new tab.
const linkClick = EditorView.domEventHandlers({
  mousedown(event, view) {
    const target = event.target as HTMLElement;
    const linkEl = target.closest(".cm-md-link");
    if (!linkEl) return false;
    const pos = view.posAtDOM(linkEl);
    let node = syntaxTree(view.state).resolveInner(pos, 1);
    while (
      node &&
      node.type.name !== "Link" &&
      node.type.name !== "Autolink" &&
      node.type.name !== "URL"
    ) {
      node = node.parent!;
    }
    if (!node) return false;
    let url: string | null = null;
    if (node.type.name === "URL") {
      url = view.state.doc.sliceString(node.from, node.to);
    } else {
      let child = node.firstChild;
      while (child) {
        if (child.type.name === "URL") {
          url = view.state.doc.sliceString(child.from, child.to);
          break;
        }
        child = child.nextSibling;
      }
    }
    if (!url) return false;
    if (url.startsWith("www.")) url = `https://${url}`;
    else if (url.includes("@") && !/^[a-z]+:/i.test(url)) url = `mailto:${url}`;
    event.preventDefault();
    void openUrl(url);
    return true;
  },
});

const WIKI_PADDING = "64px max(1.5rem, calc(50% - 24rem)) 24px";

function buildTheme(padding: string, fontSize: string, height: string, overflowY: string) {
  return EditorView.theme({
    "&": {
      backgroundColor: "transparent",
      color: "rgb(var(--color-fg))",
      height,
      fontSize,
    },
    "&.cm-focused": { outline: "none" },
    ".cm-scroller": {
      fontFamily: "inherit",
      lineHeight: "1.7",
      overflowY,
    },
    ".cm-content": {
      padding,
      caretColor: "rgb(var(--color-fg))",
    },
    ".cm-line": { padding: 0 },
  });
}

export function MarkdownEditor({
  value,
  onChange,
  onSave,
  className,
  vimMode = false,
  padding = WIKI_PADDING,
  fontSize = "14px",
  autoGrow = false,
}: {
  value: string;
  onChange: (v: string) => void;
  onSave?: () => void;
  className?: string;
  vimMode?: boolean;
  padding?: string;
  fontSize?: string;
  autoGrow?: boolean;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const cbs = useRef({ onChange, onSave });
  cbs.current = { onChange, onSave };

  useEffect(() => {
    if (!hostRef.current) return;
    const view = new EditorView({
      doc: value,
      parent: hostRef.current,
      extensions: [
        // Vim must come before other keymaps so it can override them.
        ...(vimMode ? [vim()] : []),
        saveCallback.of(() => cbs.current.onSave?.()),
        history(),
        keymap.of([
          ...defaultKeymap,
          ...historyKeymap,
          indentWithTab,
          {
            key: "Mod-s",
            preventDefault: true,
            run: () => {
              cbs.current.onSave?.();
              return true;
            },
          },
        ]),
        markdown({ base: markdownLanguage }),
        livePreview,
        tableOverlay,
        frontmatterOverlay,
        imagePreloader,
        linkClick,
        EditorView.lineWrapping,
        EditorView.updateListener.of((u) => {
          if (u.docChanged) cbs.current.onChange(u.state.doc.toString());
        }),
        buildTheme(padding, fontSize, autoGrow ? "auto" : "100%", autoGrow ? "visible" : "auto"),
      ],
    });
    viewRef.current = view;
    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Sync external value changes (e.g. switching files).
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    if (view.state.doc.toString() !== value) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: value },
      });
    }
  }, [value]);

  return <div ref={hostRef} className={className} />;
}
