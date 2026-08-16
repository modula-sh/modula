import { useEffect } from "react";
import { createPortal } from "react-dom";
import { useModalPortal } from "../contexts/ModalPortalContext";

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

  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape" && !busy) onCancel();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, busy, onCancel]);

  if (!open || !target) return null;

  return createPortal(
    <div
      className="absolute inset-0 z-50 flex items-center justify-center bg-overlay overlay-fade"
      onClick={(e) => {
        if (e.target === e.currentTarget && !busy) onCancel();
      }}
    >
      <div
        className={`bg-bg border border-border rounded-xl shadow-xl max-w-[90vw] max-h-[90vh] overflow-y-auto p-4 flex flex-col gap-3 font-inter ${panelClassName}`}
      >
        {children}
      </div>
    </div>,
    target,
  );
}
