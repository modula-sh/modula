import { Breadcrumbs } from "../components/Breadcrumbs";
import {
  useHeaderCenterActive,
  useHeaderCenterSlotRegister,
  useHeaderSlotRegister,
} from "../contexts/HeaderSlotContext";

export function Header() {
  const setSlot = useHeaderSlotRegister();
  const setCenterSlot = useHeaderCenterSlotRegister();
  const centerActive = useHeaderCenterActive();

  return (
    // When a centered slot is active (agents subnav), lay the header out as a
    // 3-column grid so the center column stays centered on the full header and
    // the breadcrumb column (minmax(0,1fr)) truncates instead of pushing it.
    // Otherwise fall back to flex so the slot can span the full remaining width.
    <header
      className={`h-12 px-4 border-b border-edge items-center gap-3 font-inter select-none ${
        centerActive ? "grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)]" : "flex"
      }`}
    >
      <div className="min-w-0">
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
