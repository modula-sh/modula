export type DiffLineKind = "context" | "add" | "del";

export interface DiffLine {
  kind: DiffLineKind;
  oldNo: number | null;
  newNo: number | null;
  content: string;
}

export interface DiffHunk {
  lines: DiffLine[];
}

const HEADER_PREFIXES = [
  "diff ",
  "index ",
  "--- ",
  "+++ ",
  "new file mode",
  "deleted file mode",
  "old mode",
  "new mode",
  "similarity index",
  "rename from",
  "rename to",
  "copy from",
  "copy to",
  "Binary files",
];

const HUNK_HEADER = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;

export function parseUnifiedDiff(text: string): DiffHunk[] {
  const hunks: DiffHunk[] = [];
  let cur: DiffHunk | null = null;
  let oldNo = 0;
  let newNo = 0;

  for (const line of text.split("\n")) {
    const hunkMatch = HUNK_HEADER.exec(line);
    if (hunkMatch) {
      oldNo = parseInt(hunkMatch[1], 10);
      newNo = parseInt(hunkMatch[2], 10);
      cur = { lines: [] };
      hunks.push(cur);
      continue;
    }
    if (!cur || HEADER_PREFIXES.some((p) => line.startsWith(p))) continue;
    if (line.startsWith("\\")) continue;

    const tag = line[0];
    const content = line.slice(1);
    if (tag === "+") {
      cur.lines.push({ kind: "add", oldNo: null, newNo, content });
      newNo++;
    } else if (tag === "-") {
      cur.lines.push({ kind: "del", oldNo, newNo: null, content });
      oldNo++;
    } else if (tag === " ") {
      cur.lines.push({ kind: "context", oldNo, newNo, content });
      oldNo++;
      newNo++;
    }
  }
  return hunks;
}
