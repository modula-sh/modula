import { useMemo } from "react";
import { type DiffHunk, type DiffLine, parseUnifiedDiff } from "../lib/diff";
import { highlightToHtml, languageFromPath } from "../lib/highlight";

export function DiffBody({ text, path }: { text: string; path?: string }) {
  const hunks = useMemo(() => parseUnifiedDiff(text), [text]);
  const language = useMemo(() => (path ? languageFromPath(path) : null), [path]);

  if (hunks.length === 0) {
    return <div className="px-3 py-2 text-fg-subtle text-xs">no changes</div>;
  }

  const gutterWidth = Math.max(
    2,
    String(Math.max(...hunks.flatMap((h) => h.lines.map((l) => l.newNo ?? l.oldNo ?? 0)))).length,
  );
  // Fit the widest line number plus the lane's own padding (pl-2 + pr-1.5),
  // so the largest number keeps a right-side gap from the lane edge.
  const numCol = `calc(${gutterWidth}ch + 0.875rem)`;

  return (
    <div className="text-[11px] font-mono leading-snug border-t border-border overflow-x-auto bg-bg dark:bg-bg/40">
      {/* Grow to the widest line so row backgrounds fill the full scroll width. */}
      <div className="w-max min-w-full">
        {hunks.map((h, i) => (
          <HunkBlock key={i} hunk={h} language={language} numCol={numCol} isFirst={i === 0} />
        ))}
      </div>
    </div>
  );
}

function HunkBlock({
  hunk,
  language,
  numCol,
  isFirst,
}: {
  hunk: DiffHunk;
  language: string | null;
  numCol: string;
  isFirst: boolean;
}) {
  return (
    <div className={isFirst ? "" : "border-t border-border/40 mt-1 pt-1"}>
      {hunk.lines.map((l, i) => (
        <LineRow key={i} line={l} language={language} numCol={numCol} />
      ))}
    </div>
  );
}

function LineRow({
  line,
  language,
  numCol,
}: {
  line: DiffLine;
  language: string | null;
  numCol: string;
}) {
  const html = highlightToHtml(line.content, language);
  // Brighter background in the line-number lane, fainter tint over the syntax.
  const gutterBg =
    line.kind === "add" ? "bg-green-500/25" : line.kind === "del" ? "bg-red-500/25" : "";
  const codeBg =
    line.kind === "add" ? "bg-green-500/10" : line.kind === "del" ? "bg-red-500/10" : "";
  return (
    <div className="grid min-w-full" style={{ gridTemplateColumns: `${numCol} 1fr` }}>
      <span className={`pl-2 pr-1.5 text-right text-fg-muted select-none ${gutterBg}`}>
        {line.newNo ?? line.oldNo ?? ""}
      </span>
      <code
        className={`selectable px-2 whitespace-pre ${codeBg}`}
        dangerouslySetInnerHTML={{ __html: html || "&nbsp;" }}
      />
    </div>
  );
}
