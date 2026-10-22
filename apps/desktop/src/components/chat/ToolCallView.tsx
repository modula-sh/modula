import { memo, useState } from "react";
import { formatToolUse } from "../../lib/logFormat";
import { toolBody } from "../../lib/toolDisplay";
import type { ToolCall } from "../../types";

const PREVIEW_LINES = 4;
const NESTED_SHOWN = 4;

const DOT: Record<ToolCall["status"], string> = {
  running: "text-fg-muted animate-pulse",
  completed: "text-fg-subtle",
  failed: "text-red-400",
};

function finished(status: ToolCall["status"]): ToolCall["status"] {
  return status === "running" ? "completed" : status;
}

function lineClass(line: string, diff: boolean): string {
  if (!diff) return "";
  if (line.startsWith("+") && !line.startsWith("+++")) return "text-green-500";
  if (line.startsWith("-") && !line.startsWith("---")) return "text-red-400";
  return "";
}

/** One tool call: the call, any calls a sub-agent made under it, and a preview of its result. */
export const ToolCallView = memo(function ToolCallView({
  tool,
  nested,
  live = false,
}: {
  tool: ToolCall;
  nested?: ToolCall[];
  /** Part of the reply in flight; a stored call left running was cut short. */
  live?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [allNested, setAllNested] = useState(false);
  const { primary, continuation } = formatToolUse(tool.name, tool.input);
  const { lines, diff } = toolBody(tool);
  const shown = open ? lines : lines.slice(0, PREVIEW_LINES);
  const hidden = lines.length - PREVIEW_LINES;
  const calls = nested ?? [];
  const shownCalls = allNested ? calls : calls.slice(-NESTED_SHOWN);

  return (
    <div className="font-mono text-[13px] min-w-0">
      <div className="flex gap-2 text-fg-muted">
        <span className={live ? DOT[tool.status] : DOT[finished(tool.status)]}>●</span>
        <span className="min-w-0 [overflow-wrap:anywhere]">{primary}</span>
      </div>
      {continuation.map((line, i) => (
        <div key={i} className="flex gap-2 pl-5 text-fg-subtle">
          <span>⎿</span>
          <span className="min-w-0 whitespace-pre-wrap [overflow-wrap:anywhere]">{line}</span>
        </div>
      ))}
      {calls.length > 0 && (
        <div className="ml-[5px] mt-1 pl-4 border-l border-edge flex flex-col gap-1">
          {calls.length > shownCalls.length && (
            <button
              type="button"
              onClick={() => setAllNested(true)}
              className="text-left text-fg-subtle hover:text-fg"
            >
              … +{calls.length - shownCalls.length} earlier tool uses
            </button>
          )}
          {shownCalls.map((call) => (
            <ToolCallView key={call.id} tool={call} live={live} />
          ))}
        </div>
      )}
      {shown.length > 0 && (
        <div
          className={`flex gap-2 pl-5 ${tool.status === "failed" ? "text-red-400" : "text-fg-subtle"}`}
        >
          <span>⎿</span>
          <pre
            className={`selectable min-w-0 flex-1 whitespace-pre-wrap [overflow-wrap:anywhere] ${
              open ? "max-h-96 overflow-y-auto" : ""
            }`}
          >
            {shown.map((line, i) => (
              <div key={i} className={lineClass(line, diff)}>
                {line || " "}
              </div>
            ))}
          </pre>
        </div>
      )}
      {hidden > 0 && (
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          className="pl-[42px] text-fg-subtle hover:text-fg"
        >
          {open ? "Show less" : `… +${hidden} lines`}
          {open && tool.output_truncated ? " (output truncated)" : ""}
        </button>
      )}
    </div>
  );
});
