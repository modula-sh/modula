import { ChevronLeft, ChevronRight, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { useLocation, useNavigate } from "react-router-dom";
import { IconButton } from "../components/IconButton";
import { WindowControls } from "../components/WindowControls";
import { useSidebarContext } from "../contexts/SidebarContext";
import { windowButtons } from "../tauri/window";

/** Window-level chrome: navigation, the platform window buttons, and the drag
 * region between them. 35px matches VS Code's title bar. */
export function Titlebar() {
  const { open, toggle } = useSidebarContext();
  const navigate = useNavigate();
  const location = useLocation();

  // `location` is read so back/forward recompute on every navigation.
  const historyIdx = (window.history.state?.idx as number | undefined) ?? 0;
  void location;

  return (
    // z-60 keeps the window buttons live above modals; pl clears the macOS lights.
    <header
      className={`relative z-[60] shrink-0 h-[35px] flex items-center gap-1 font-inter select-none ${windowButtons() === "system" ? "pl-[84px]" : "pl-2"}`}
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
