import type { ToolCall } from "../types";
import { oneLine } from "./logFormat";

/** Each provider's shell tool; its input's `command` is what ran. */
const SHELLS = new Set([
  "Bash",
  "bash",
  "run_shell_command",
  "commandExecution",
  "command_execution",
]);

/** The input field holding the file a write tool writes, per provider tool. */
const WRITES: Record<string, string> = {
  Write: "content",
  write: "content",
  write_file: "content",
};

/** The input fields holding an edit tool's old and new text, per provider tool. */
const EDITS: Record<string, [string, string]> = {
  Edit: ["old_string", "new_string"],
  edit: ["oldString", "newString"],
  replace: ["old_string", "new_string"],
};

function prefixed(text: string, sign: string): string[] {
  return text.split("\n").map((line) => sign + line);
}

/** What to show under a call: what it wrote, the edit it made, else what it returned. */
export function toolBody(tool: ToolCall): { lines: string[]; diff: boolean } {
  const input = (tool.input && typeof tool.input === "object" ? tool.input : {}) as Record<
    string,
    unknown
  >;
  const output = { lines: lines(tool.output ?? ""), diff: false };
  // A command the one-line title had to cut is shown whole, ahead of its output.
  const command = SHELLS.has(tool.name) ? input.command : undefined;
  if (typeof command === "string" && oneLine(command) !== command) {
    const [first = "", ...rest] = lines(command);
    return { lines: [`$ ${first}`, ...rest, ...output.lines], diff: false };
  }
  if (tool.status === "failed") return output;

  const written = input[WRITES[tool.name]];
  if (typeof written === "string") return { lines: lines(written), diff: false };

  const [oldKey, newKey] = EDITS[tool.name] ?? [];
  const before = input[oldKey];
  const after = input[newKey];
  if (typeof before === "string" && typeof after === "string") {
    return { lines: [...prefixed(before, "-"), ...prefixed(after, "+")], diff: true };
  }

  if (tool.name === "fileChange" && Array.isArray(input.changes)) {
    const diffs = (input.changes as { diff?: unknown }[]).map((c) =>
      typeof c.diff === "string" ? c.diff : "",
    );
    return { lines: lines(diffs.join("\n")), diff: true };
  }
  return output;
}

function lines(text: string): string[] {
  const trimmed = text.replace(/\n+$/, "");
  return trimmed ? trimmed.split("\n") : [];
}

/** `21s`, `1m 7s`, `1h 2m`. */
export function formatDuration(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}
