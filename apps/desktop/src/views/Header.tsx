import { ChevronLeft, ChevronRight, PanelLeftOpen } from "lucide-react";
import { useLocation, useNavigate } from "react-router-dom";
import { Breadcrumbs } from "../components/Breadcrumbs";
import { IconButton } from "../components/IconButton";
import {
  useHeaderCenterActive,
  useHeaderCenterSlotRegister,
  useHeaderSlotRegister,
} from "../contexts/HeaderSlotContext";
import { useSidebarContext } from "../contexts/SidebarContext";
import { safePlatform } from "../tauri/platform";

export function Header() {
  const { open: sidebarOpen, toggle: toggleSidebar } = useSidebarContext();
  const setSlot = useHeaderSlotRegister();
  const setCenterSlot = useHeaderCenterSlotRegister();
  const centerActive = useHeaderCenterActive();
  const navigate = useNavigate();
  const location = useLocation();
  const isWindows = safePlatform() === "windows";
  // macOS needs pl-12 to clear the traffic lights; Windows has none.
  const leftPad = sidebarOpen ? "pl-4" : isWindows ? "pl-0" : "pl-12";

  const historyIdx = (window.history.state?.idx as number | undefined) ?? 0;
  void location;
  const canGoBack = historyIdx > 0;
  const canGoForward = historyIdx < window.history.length - 1;
  return (
    // When a centered slot is active (agents subnav), lay the header out as a
    // 3-column grid so the center column stays centered on the full header and
    // the breadcrumb column (minmax(0,1fr)) truncates instead of pushing it.
    // Otherwise fall back to flex so the slot can span the full remaining width.
    <header
      className={`h-12 ${leftPad} pr-4 border-b border-border items-center gap-3 font-inter select-none transition-[padding] duration-200 ease-out ${
        centerActive ? "grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)]" : "flex"
      }`}
    >
      <div className="flex items-center gap-3 min-w-0">
        {!sidebarOpen && (
          // -ml-10 matches the open sidebar's button position (no shift on toggle).
          <div className={`flex items-center gap-1 shrink-0 ${isWindows ? "-ml-10" : ""}`}>
            <IconButton onClick={() => navigate(-1)} disabled={!canGoBack} title="Back">
              <ChevronLeft size={16} />
            </IconButton>
            <IconButton onClick={() => navigate(1)} disabled={!canGoForward} title="Forward">
              <ChevronRight size={16} />
            </IconButton>
            <IconButton
              onClick={toggleSidebar}
              aria-expanded={sidebarOpen}
              aria-controls="sidebar"
              title="Expand sidebar"
            >
              <PanelLeftOpen size={16} />
            </IconButton>
          </div>
        )}
        <Breadcrumbs />
      </div>
      <div
        ref={setCenterSlot}
        className={`${centerActive ? "flex" : "hidden"} items-center justify-center gap-1 min-w-0`}
      />
      <div
        ref={setSlot}
        className={`${centerActive ? "justify-end" : "flex-1"} flex items-center gap-2 min-w-0`}
      />
    </header>
  );
}
