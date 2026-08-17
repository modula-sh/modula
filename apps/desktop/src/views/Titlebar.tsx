import { ChevronLeft, ChevronRight, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { useLocation, useNavigate } from "react-router-dom";
import { IconButton } from "../components/IconButton";
import { WindowControls } from "../components/WindowControls";
import { useSidebarContext } from "../contexts/SidebarContext";
import { windowButtons } from "../tauri/window";

/** The app's own title bar, spanning the window above the sidebar and the
 * content card. Everything that belongs to the window rather than to a view
 * lives here: the platform's window buttons at the ends and navigation next to
 * them. The empty stretch between them is the drag region (useTitlebarDrag).
 *
 * 35px matches VS Code's title bar. Three other places follow this height —
 * WindowControls' buttons, useTitlebarDrag's grab zone, and (on macOS) the
 * traffic-light offset in tauri.conf.json.
 *
 * Only the ends are platform-specific — macOS leaves a gutter for the overlay
 * traffic lights, Windows fills the right end with WindowControls, and Linux
 * keeps its native decorations above this row. */
export function Titlebar() {
  const { open, toggle } = useSidebarContext();
  const navigate = useNavigate();
  const location = useLocation();

  // react-router stamps a position index on history.state; use it (plus the
  // session history length) to know whether back/forward have anywhere to go.
  // `location` is referenced so this recomputes on every navigation.
  const historyIdx = (window.history.state?.idx as number | undefined) ?? 0;
  void location;

  return (
    // Above the modal overlay (z-50), so the window buttons stay live while a
    // modal is open — as the OS-drawn macOS traffic lights already do.
    <header
      className={`relative z-[60] shrink-0 h-[35px] flex items-center gap-1 font-inter select-none ${windowButtons() === "system" ? "pl-[78px]" : "pl-2"}`}
    >
      <IconButton onClick={() => navigate(-1)} disabled={historyIdx <= 0} title="Back">
        <ChevronLeft size={16} />
      </IconButton>
      <IconButton
        onClick={() => navigate(1)}
        disabled={historyIdx >= window.history.length - 1}
        title="Forward"
      >
        <ChevronRight size={16} />
      </IconButton>
      <IconButton
        onClick={toggle}
        aria-expanded={open}
        aria-controls="sidebar"
        title={open ? "Collapse sidebar" : "Expand sidebar"}
      >
        {open ? <PanelLeftClose size={16} /> : <PanelLeftOpen size={16} />}
      </IconButton>
      <div className="flex-1" />
      <WindowControls />
    </header>
  );
}
