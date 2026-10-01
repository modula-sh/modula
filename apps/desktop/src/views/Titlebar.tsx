import { ChevronLeft, ChevronRight, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { useState } from "react";
import { NavigationType, useLocation, useNavigate, useNavigationType } from "react-router-dom";
import { IconButton } from "../components/IconButton";
import { WindowControls } from "../components/WindowControls";
import { useSidebarContext } from "../contexts/SidebarContext";
import { windowButtons } from "../tauri/window";

/** Window-level chrome: navigation, the platform window buttons, and the drag
 * region between them. 35px matches VS Code's title bar. */
export function Titlebar() {
  const { open, toggle } = useSidebarContext();
  const navigate = useNavigate();
  const { canBack, canForward } = useHistoryBounds();

  return (
    // z-60 keeps the window buttons live above modals; pl clears the macOS lights.
    <header
      className={`relative z-[60] shrink-0 h-[35px] flex items-center gap-1 font-inter select-none ${windowButtons() === "system" ? "pl-[84px]" : "pl-2"}`}
    >
      <IconButton onClick={() => navigate(-1)} disabled={!canBack} title="Back">
        <ChevronLeft size={16} />
      </IconButton>
      <IconButton onClick={() => navigate(1)} disabled={!canForward} title="Forward">
        <ChevronRight size={16} />
      </IconButton>
      <IconButton
        onClick={toggle}
        aria-expanded={open}
        aria-controls="sidebar"
        title={open ? "Collapse sidebar" : "Expand sidebar"}
      >
        {open ? (
          <PanelLeftClose size={16} className="icon-crisp" />
        ) : (
          <PanelLeftOpen size={16} className="icon-crisp" />
        )}
      </IconButton>
      <div className="flex-1" />
      <WindowControls />
    </header>
  );
}

/** Back/forward availability. The workspace's memory router keeps its history
 * index private, so mirror the entry stack from location keys. */
function useHistoryBounds() {
  const { key } = useLocation();
  const action = useNavigationType();
  const [h, setH] = useState({ keys: [key], idx: 0 });
  if (h.keys[h.idx] !== key) {
    if (action === NavigationType.Push) {
      setH({ keys: [...h.keys.slice(0, h.idx + 1), key], idx: h.idx + 1 });
    } else if (action === NavigationType.Replace) {
      setH({ keys: h.keys.map((k, i) => (i === h.idx ? key : k)), idx: h.idx });
    } else {
      const idx = h.keys.indexOf(key);
      setH(idx >= 0 ? { keys: h.keys, idx } : { keys: [key], idx: 0 });
    }
  }
  return { canBack: h.idx > 0, canForward: h.idx < h.keys.length - 1 };
}
