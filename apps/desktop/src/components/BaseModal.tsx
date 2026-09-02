import { useEffect } from "react";
import { createPortal } from "react-dom";
import { useModalPortal } from "../contexts/ModalPortalContext";

const CHROME = "bg-bg border border-edge rounded-xl shadow-xl p-4 flex flex-col gap-3";

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
      className={`absolute inset-0 z-50 flex justify-center bg-overlay overlay-fade ${
        align === "top" ? "items-start pt-[12vh]" : "items-center"
      }`}
      onClick={(e) => {
        if (e.target === e.currentTarget && !busy) onCancel();
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
