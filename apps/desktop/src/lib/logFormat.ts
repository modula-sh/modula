const MAX = 140;

export function tildifyPath(s: string): string {
  return s.replace(/^\/Users\/[^/]+\//, "~/");
}

export function oneLine(s: string, max = MAX): string {
  const flat = s
    .replace(/\r?\n|\r/g, " ")
    .replace(/ {2,}/g, " ")
    .trim();
  return flat.length > max ? `${flat.slice(0, max)}…` : flat;
}

export interface FormatResult {
  primary: string;
  continuation: string[];
}

type TodoItem = { content: string; status: string };

const TODO_GLYPHS: Record<string, string> = {
  pending: "☐",
  in_progress: "◐",
  completed: "☒",
};

function todoLines(items: TodoItem[]): string[] {
  const lines = items.map((t) => `${TODO_GLYPHS[t.status] ?? "☐"} ${oneLine(t.content)}`);
  if (lines.length <= 8) return lines;
  return [...lines.slice(0, 8), `… +${lines.length - 8} more`];
}

function firstScalar(input: unknown): string {
  if (typeof input === "string") return oneLine(input);
  if (input && typeof input === "object") {
    for (const v of Object.values(input as Record<string, unknown>)) {
      if (typeof v === "string") return oneLine(v);
    }
  }
  return "";
}

export function formatToolUse(name: string, input: unknown): FormatResult {
  const i = (input && typeof input === "object" ? input : {}) as Record<string, unknown>;

  switch (name) {
    case "Bash": {
      const cmd = typeof i.command === "string" ? oneLine(i.command) : "";
      const desc = typeof i.description === "string" ? oneLine(i.description) : "";
      return { primary: `Bash(${cmd})`, continuation: desc ? [desc] : [] };
    }
    case "Read": {
      const fp = typeof i.file_path === "string" ? tildifyPath(i.file_path) : "";
      return { primary: `Read(${fp})`, continuation: [] };
    }
    case "Edit": {
      const fp = typeof i.file_path === "string" ? tildifyPath(i.file_path) : "";
      const cont = i.replace_all === true ? ["replace_all: true"] : [];
      return { primary: `Update(${fp})`, continuation: cont };
    }
    case "Write": {
      const fp = typeof i.file_path === "string" ? tildifyPath(i.file_path) : "";
      return { primary: `Write(${fp})`, continuation: [] };
    }
    case "Glob": {
      const pattern = typeof i.pattern === "string" ? oneLine(i.pattern) : "";
      const cont = typeof i.path === "string" ? [`in ${tildifyPath(i.path)}`] : [];
      return { primary: `Glob(${pattern})`, continuation: cont };
    }
    case "Grep": {
      const pattern = typeof i.pattern === "string" ? oneLine(i.pattern) : "";
      const cont: string[] = [];
      if (typeof i.path === "string") cont.push(`in ${tildifyPath(i.path)}`);
      if (typeof i.glob === "string") cont.push(`--glob ${i.glob}`);
      if (typeof i.type === "string") cont.push(`--type ${i.type}`);
      return { primary: `Grep(${pattern})`, continuation: cont };
    }
    case "TodoWrite": {
      const todos = Array.isArray(i.todos) ? (i.todos as TodoItem[]) : [];
      return {
        primary: `TodoWrite(${todos.length} item${todos.length === 1 ? "" : "s"})`,
        continuation: todoLines(todos),
      };
    }
    case "Agent": {
      const desc = typeof i.description === "string" ? oneLine(i.description) : "";
      const cont = typeof i.subagent_type === "string" ? [`subagent_type: ${i.subagent_type}`] : [];
      return { primary: `Agent(${desc})`, continuation: cont };
    }
    case "ToolSearch": {
      const query = typeof i.query === "string" ? oneLine(i.query) : "";
      return { primary: `ToolSearch(${query})`, continuation: [] };
    }
    default: {
      const val = firstScalar(input);
      return { primary: `${name}(${val})`, continuation: [] };
    }
  }
}
