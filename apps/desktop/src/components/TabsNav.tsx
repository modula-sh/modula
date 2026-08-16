import { useEffect } from "react";
import { NavLink } from "react-router-dom";
import { useHeaderCenterActivate } from "../contexts/HeaderSlotContext";
import { HeaderCenterSlot, HeaderSlot } from "./HeaderSlot";

/** Sub-tab nav for /agents, /agents/logs, /agents/usage. The tabs render into the
 * header's centered slot so they stay centered on the full header regardless of
 * breadcrumb width; the optional `right` node renders into the main header slot.
 * NavLink derives the active state from the URL; `end` is set so the parent tab
 * doesn't stay marked active on a sibling sub-route. */
export function TabsNav({ right }: { right?: React.ReactNode } = {}) {
  const setCenterActive = useHeaderCenterActivate();
  useEffect(() => {
    setCenterActive(true);
    return () => setCenterActive(false);
  }, [setCenterActive]);

  return (
    <>
      <HeaderCenterSlot>
        <div className="flex items-center justify-center gap-1">
          <Tab to="/agents" end>
            Agents
          </Tab>
          <Tab to="/agents/logs">Runs</Tab>
          <Tab to="/agents/usage">Usage</Tab>
        </div>
      </HeaderCenterSlot>
      {right && <HeaderSlot>{right}</HeaderSlot>}
    </>
  );
}

function Tab({ to, end, children }: { to: string; end?: boolean; children: React.ReactNode }) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        "px-2.5 py-1 rounded text-xs font-inter uppercase tracking-wide transition-colors " +
        (isActive ? "bg-surface-2 text-fg" : "text-fg-subtle hover:text-fg hover:bg-surface")
      }
    >
      {children}
    </NavLink>
  );
}
