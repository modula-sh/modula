import { useEffect, useId } from "react";
import { createPortal } from "react-dom";
import { useModalPortal } from "../contexts/ModalPortalContext";

const CHROME = "bg-bg border border-edge rounded-xl shadow-xl p-4 flex flex-col gap-3";

// Open modals, outermost first. Only the last one reacts to Escape, so nesting
// (an AI prompt over the New Task modal) doesn't dismiss both.
const stack: string[] = [];

/** Modal shell — portals into the nearest `ModalPortalProvider`. */
export function BaseModal({
  open,
  busy = false,
  onCancel,
  children,
  align = "center",
  chromeless = false,
  panelClassName = "w-[28rem]",
}: {
  open: boolean;
  busy?: boolean;
  onCancel: () => void;
  children: React.ReactNode;
  /** `top` anchors the panel near the top of the window (command-palette style). */
  align?: "center" | "top";
  /** Skip the default panel surface so `panelClassName` supplies its own. */
  chromeless?: boolean;
  /** Override the inner panel sizing (default is `w-[28rem]`). */
  panelClassName?: string;
}) {
  const target = useModalPortal();
  const id = useId();
  // Own effect: a re-render with a fresh `onCancel` must not re-push and promote
  // this modal above a nested one.
  useEffect(() => {
    if (!open) return;
    stack.push(id);
    return () => {
      stack.splice(stack.indexOf(id), 1);
    };
  }, [open, id]);

  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape" && !busy && stack[stack.length - 1] === id) onCancel();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, busy, onCancel, id]);

  if (!open || !target) return null;

  return createPortal(
    <div
      className={`absolute inset-0 z-50 flex justify-center bg-overlay overlay-fade ${
        align === "top" ? "items-start pt-[12vh]" : "items-center"
      }`}
      onClick={(e) => {
        if (e.target === e.currentTarget && !busy && stack[stack.length - 1] === id) onCancel();
      }}
    >
      <div
        className={`max-w-[90vw] max-h-[90vh] overflow-y-auto font-inter ${
          chromeless ? "" : CHROME
        } ${panelClassName}`}
      >
        {children}
      </div>
    </div>,
    target,
  );
}
