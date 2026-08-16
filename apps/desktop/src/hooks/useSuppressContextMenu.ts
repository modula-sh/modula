import { useEffect } from "react";

// Editable regions keep the native menu so right-click cut/copy/paste works.
function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const el = target.closest("input, textarea, [contenteditable]");
  if (!el) return false;
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    return !el.disabled && !el.readOnly;
  }
  return el.getAttribute("contenteditable") !== "false";
}

// Suppress the webview's default context menu app-wide; custom menus opt out via preventDefault.
export function useSuppressContextMenu(): void {
  useEffect(() => {
    const onContextMenu = (e: MouseEvent) => {
      if (isEditableTarget(e.target)) return;
      e.preventDefault();
    };
    document.addEventListener("contextmenu", onContextMenu);
    return () => document.removeEventListener("contextmenu", onContextMenu);
  }, []);
}
