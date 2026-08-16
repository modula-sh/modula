import { formatToolUse } from "../../lib/logFormat";

export function ToolUseLine({ name, input }: { name: string; input: unknown }) {
  const { primary, continuation } = formatToolUse(name, input);
  return (
    <div>
      <div className="text-fg-muted">
        <span className="text-fg-subtle">→ </span>
        <span className="font-mono text-[13px]">{primary}</span>
      </div>
      {continuation.map((line, i) => (
        <div key={i} className="text-fg-subtle pl-4">
          <span> ⎿ </span>
          <span className="font-mono text-[13px] whitespace-pre-wrap">{line}</span>
        </div>
      ))}
    </div>
  );
}
