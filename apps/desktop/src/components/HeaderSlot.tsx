import { createPortal } from "react-dom";
import { useHeaderCenterSlotElement, useHeaderSlotElement } from "../contexts/HeaderSlotContext";

/** Renders `children` into the main header slot registered by `<Header>`. */
export function HeaderSlot({ children }: { children: React.ReactNode }) {
  const el = useHeaderSlotElement();
  if (!el) return null;
  return createPortal(children, el);
}

/** Renders `children` into the header's centered slot — centered relative to the
 * full header, independent of the breadcrumb width. */
export function HeaderCenterSlot({ children }: { children: React.ReactNode }) {
  const el = useHeaderCenterSlotElement();
  if (!el) return null;
  return createPortal(children, el);
}
