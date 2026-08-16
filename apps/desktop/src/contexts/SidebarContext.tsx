import { createContext, useContext } from "react";
import { useSidebar } from "../hooks/useSidebar";

interface SidebarState {
  open: boolean;
  setOpen: (v: boolean) => void;
  toggle: () => void;
  /** Layout reports the live nav+content region width; drives auto-collapse. */
  notifyRegionWidth: (width: number) => void;
}

const SidebarContext = createContext<SidebarState>({
  open: true,
  setOpen: () => {},
  toggle: () => {},
  notifyRegionWidth: () => {},
});

export function SidebarProvider({ children }: { children: React.ReactNode }) {
  const value = useSidebar();
  return <SidebarContext.Provider value={value}>{children}</SidebarContext.Provider>;
}

export function useSidebarContext(): SidebarState {
  return useContext(SidebarContext);
}
