import { useEffect, useId } from "react";
import { createPortal } from "react-dom";
import { useModalPortal } from "../contexts/ModalPortalContext";

// Open modals, outermost first. Nested modals (an AI prompt over the New Task
// modal) must not both dismiss on one Escape, so only the last one reacts.
const stack: string[] = [];

/** Modal shell — portals into the nearest `ModalPortalProvider`. */
export function BaseModal({
  open,
  busy = false,
  onCancel,
  children,
  panelClassName = "w-[28rem]",
}: {
  open: boolean;
  busy?: boolean;
  onCancel: () => void;
  children: React.ReactNode;
  /** Override the inner panel sizing (default is `w-[28rem]`). */
  panelClassName?: string;
}) {
  const target = useModalPortal();
  const id = useId();
  // Own effect so a re-render with a fresh `onCancel` doesn't re-push and
  // wrongly promote this modal above a nested one.
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
      className="absolute inset-0 z-50 flex items-center justify-center bg-overlay overlay-fade"
      onClick={(e) => {
        if (e.target === e.currentTarget && !busy && stack[stack.length - 1] === id) onCancel();
      }}
    >
      <div
        className={`bg-bg border border-edge rounded-xl shadow-xl max-w-[90vw] max-h-[90vh] overflow-y-auto p-4 flex flex-col gap-3 font-inter ${panelClassName}`}
      >
        {children}
      </div>
    </div>,
    target,
  );
}
