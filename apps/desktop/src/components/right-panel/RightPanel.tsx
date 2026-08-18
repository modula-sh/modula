import { X } from "lucide-react";
import { type RightPanelContent, useRightPanel } from "../../contexts/RightPanelProvider";
import { IconButton } from "../IconButton";
import { BranchDiffPanel } from "./BranchDiffPanel";
import { DiffPanel } from "./DiffPanel";

// Cap to a fraction of the viewport so the panel yields on narrow windows
// instead of crushing the content column.
const PANEL_WIDTH = "w-[min(640px,55vw)]";

export function RightPanel() {
  const { state, close } = useRightPanel();
  if (!state.open || !state.content) return null;
  return (
    <aside className={`${PANEL_WIDTH} shrink-0 flex flex-col border-l border-edge`}>
      <div className="shrink-0 h-12 flex items-center justify-between gap-2 px-3 border-b border-edge">
        <span className="text-xs text-fg font-inter font-medium truncate">
          {state.title ?? <PanelTitle content={state.content} />}
        </span>
        <span className="flex items-center gap-2 ml-auto shrink-0">
          {state.action}
          <IconButton onClick={close} title="Close panel">
            <X size={16} />
          </IconButton>
        </span>
      </div>
      <div className="flex-1 overflow-y-auto">
        <PanelBody content={state.content} />
      </div>
    </aside>
  );
}

function PanelTitle({ content }: { content: RightPanelContent }) {
  switch (content.type) {
    // Placeholder for both: each panel sets the real "Diff <branch>" title
    // once its fetch resolves the branch name.
    case "diff":
    case "branch-diff":
      return <span className="text-fg-subtle">Diff</span>;
  }
}

function PanelBody({ content }: { content: RightPanelContent }) {
  switch (content.type) {
    case "diff":
      return <DiffPanel {...content} />;
    case "branch-diff":
      return <BranchDiffPanel {...content} />;
  }
}
