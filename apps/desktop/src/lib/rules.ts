// Pure conversion between the wire format for agent rules (`string[]`, one
// expression per entry, OR-ed by the dispatcher) and a structured builder model
// (rows of OR, each an AND-chain of `key op value` comparisons).
//
// The grammar lives in apps/engine/src/services/dispatcher/expr.rs. Only a flat
// AND-chain of `path (==|!=) <string|true|false>` round-trips; OR, parens, bare
// paths, or path-vs-path RHS are kept verbatim as raw-only rows (Raw mode is the
// escape hatch). Parsing never throws.

import { type RuleOp, ruleKeyFor } from "./rulesSchema";

export interface RuleComparison {
  key: string;
  op: RuleOp;
  value: string;
}

export interface RuleRow {
  comparisons: RuleComparison[];
  // Set only when the source entry could not be reduced to comparisons; the row
  // then round-trips its original text verbatim.
  raw?: string;
}

export type RuleModel = RuleRow[];

type Tok =
  | { t: "ident"; v: string }
  | { t: "str"; v: string }
  | { t: "dot" }
  | { t: "lparen" }
  | { t: "rparen" }
  | { t: "eq" }
  | { t: "ne" }
  | { t: "and" }
  | { t: "or" };

function tokenize(src: string): Tok[] | null {
  const out: Tok[] = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (/\s/.test(c)) {
      i += 1;
    } else if (c === ".") {
      out.push({ t: "dot" });
      i += 1;
    } else if (c === "(") {
      out.push({ t: "lparen" });
      i += 1;
    } else if (c === ")") {
      out.push({ t: "rparen" });
      i += 1;
    } else if (c === "=" && src[i + 1] === "=") {
      out.push({ t: "eq" });
      i += 2;
    } else if (c === "!" && src[i + 1] === "=") {
      out.push({ t: "ne" });
      i += 2;
    } else if (c === '"' || c === "'") {
      const end = src.indexOf(c, i + 1);
      if (end === -1) return null;
      out.push({ t: "str", v: src.slice(i + 1, end) });
      i = end + 1;
    } else if (/[A-Za-z_]/.test(c)) {
      let j = i + 1;
      while (j < src.length && /[A-Za-z0-9_]/.test(src[j])) j += 1;
      const word = src.slice(i, j);
      out.push(
        word === "and" ? { t: "and" } : word === "or" ? { t: "or" } : { t: "ident", v: word },
      );
      i = j;
    } else {
      return null;
    }
  }
  return out;
}

// Parse one AND-segment of tokens as `path (== | !=) <string | true | false>`;
// null if it does not fit that exact shape.
function parseComparison(toks: Tok[]): RuleComparison | null {
  let i = 0;
  if (toks[i]?.t !== "ident") return null;
  const path: string[] = [(toks[i] as { v: string }).v];
  i += 1;
  while (toks[i]?.t === "dot") {
    const next = toks[i + 1];
    if (next?.t !== "ident") return null;
    path.push(next.v);
    i += 2;
  }
  const op = toks[i];
  if (op?.t !== "eq" && op?.t !== "ne") return null;
  i += 1;
  // RHS is a quoted string or a bare true/false; anything else (e.g. a path)
  // isn't builder-representable and falls back to a raw row.
  const rhs = toks[i];
  let value: string;
  if (rhs?.t === "str") value = rhs.v;
  else if (rhs?.t === "ident" && (rhs.v === "true" || rhs.v === "false")) value = rhs.v;
  else return null;
  i += 1;
  if (i !== toks.length) return null;
  return { key: path.join("."), op: op.t === "eq" ? "==" : "!=", value };
}

function parseEntry(entry: string): RuleRow {
  const rawRow: RuleRow = { comparisons: [], raw: entry };
  const toks = tokenize(entry);
  if (!toks || toks.length === 0) return rawRow;
  if (toks.some((t) => t.t === "or" || t.t === "lparen" || t.t === "rparen")) return rawRow;

  const segments: Tok[][] = [[]];
  for (const tok of toks) {
    if (tok.t === "and") segments.push([]);
    else segments[segments.length - 1].push(tok);
  }

  const comparisons: RuleComparison[] = [];
  for (const seg of segments) {
    const cmp = parseComparison(seg);
    if (!cmp) return rawRow;
    comparisons.push(cmp);
  }
  return { comparisons };
}

// Split the textarea's newline-joined string into the wire-format array. Shared
// by the builder and the agent form's buildBody() so the two never diverge.
export function linesToRules(text: string): string[] {
  return text
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean);
}

export function parseRules(rules: string[]): RuleModel {
  return rules
    .map((r) => r.trim())
    .filter(Boolean)
    .map(parseEntry);
}

function quote(value: string): string {
  return value.includes("'") && !value.includes('"') ? `"${value}"` : `'${value}'`;
}

// "bool" keys serialise true/false bare (`approved == true`); everything else is
// quoted. A non-bool value on a bool key (hand-edited Raw) falls back to quoting.
function serializeComparison(c: RuleComparison): string {
  const key = ruleKeyFor(c.key);
  const bare = key?.valueKind === "bool" && (c.value === "true" || c.value === "false");
  return `${c.key} ${c.op} ${bare ? c.value : quote(c.value)}`;
}

function serializeRow(row: RuleRow): string {
  if (row.raw != null) return row.raw;
  return row.comparisons.map(serializeComparison).join(" and ");
}

export function serializeRules(model: RuleModel): string[] {
  return model.map(serializeRow).filter((s) => s.trim() !== "");
}
