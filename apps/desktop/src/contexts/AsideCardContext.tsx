import { createContext, useContext, useState } from "react";
import { createPortal } from "react-dom";

const Ctx = createContext<{ el: HTMLElement | null; setEl: (el: HTMLElement | null) => void }>({
  el: null,
  setEl: () => {},
});

/** A second content card beside the main one, filled by whichever route wants
 * one. The target sits outside the main card because that is the whole point. */
export function AsideCardProvider({ children }: { children: React.ReactNode }) {
  const [el, setEl] = useState<HTMLElement | null>(null);
  return <Ctx.Provider value={{ el, setEl }}>{children}</Ctx.Provider>;
}

/** `display: contents` generates no box, so an unused target costs nothing. */
export function AsideCardTarget() {
  return <div ref={useContext(Ctx).setEl} className="contents" />;
}

/** Same chrome as the main card, separated by the window's own gap. */
export function AsideCard({ children }: { children: React.ReactNode }) {
  const { el } = useContext(Ctx);
  if (!el) return null;
  return createPortal(
    <aside className="w-72 shrink-0 ml-2 overflow-y-auto divide-y divide-edge rounded-xl border border-edge bg-bg shadow-content">
      {children}
    </aside>,
    el,
  );
}
