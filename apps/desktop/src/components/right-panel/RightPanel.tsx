import { X } from "lucide-react";
import {
  type RightPanelContent,
  type RightPanelPlacement,
  useRightPanel,
} from "../../contexts/RightPanelProvider";
import { IconButton } from "../IconButton";
import { BranchDiffPanel } from "./BranchDiffPanel";
import { DiffPanel } from "./DiffPanel";

// Cap to a fraction of the viewport so the panel yields on narrow windows
// instead of crushing the content column.
const PANEL_WIDTH = "w-[min(640px,55vw)]";

/** Panel inside the main content card, split off by a divider. */
export function RightPanel() {
  return <Panel placement="inline" className="border-l border-edge" />;
}

/** Panel as its own card beside the main one, separated by the window's gap. */
export function RightPanelCard() {
  return (
    <Panel
      placement="card"
      className="ml-2 rounded-xl border border-edge bg-bg shadow-content overflow-hidden"
    />
  );
}

function Panel({ placement, className }: { placement: RightPanelPlacement; className: string }) {
  const { state, close } = useRightPanel();
  if (!state.open || !state.content || state.placement !== placement) return null;
  return (
    <aside className={`${PANEL_WIDTH} shrink-0 flex flex-col ${className}`}>
      <div className="shrink-0 h-12 flex items-center justify-between gap-2 px-3 border-b border-edge">
        <span className="text-xs text-fg font-inter font-medium truncate">
          {state.title ?? <PanelTitle content={state.content} />}
        </span>
        <IconButton onClick={close} title="Close panel" className="ml-auto">
          <X size={16} />
        </IconButton>
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
