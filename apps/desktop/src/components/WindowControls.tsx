import { useEffect, useState } from "react";
import { watchMaximized, windowAction, windowButtons } from "../tauri/window";

// Windows and Linux run undecorated (src-tauri/tauri.{windows,linux}.conf.json),
// so the app draws its own caption buttons — the counterpart to the macOS
// traffic lights. Both live in the Titlebar, one at each end. 46px wide is the
// Windows caption-button width; the height matches the Titlebar row.
const BUTTON = "w-[46px] h-[35px]";

export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (windowButtons() !== "app") return;
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void watchMaximized(setMaximized).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  if (windowButtons() !== "app") return null;

  return (
    <div className="shrink-0 flex items-stretch">
      <CaptionButton onClick={() => windowAction("minimize")} title="Minimize">
        <Glyph d="M1 6 h10" />
      </CaptionButton>
      <CaptionButton
        onClick={() => windowAction("toggle-maximize")}
        title={maximized ? "Restore" : "Maximize"}
      >
        {maximized ? (
          <Glyph d="M1.5 4.5 h6 v6 h-6 z M4.5 4.5 v-3 h6 v6 h-3" />
        ) : (
          <Glyph d="M1.5 1.5 h9 v9 h-9 z" />
        )}
      </CaptionButton>
      <CaptionButton onClick={() => windowAction("close")} title="Close" close>
        <Glyph d="M1.5 1.5 l9 9 M10.5 1.5 l-9 9" />
      </CaptionButton>
    </div>
  );
}

function CaptionButton({
  children,
  onClick,
  title,
  close = false,
}: {
  children: React.ReactNode;
  onClick: () => void;
  title: string;
  close?: boolean;
}) {
  // Close keeps the platform's red hover so the destructive one reads as such.
  const hover = close ? "hover:bg-red-600 hover:text-white" : "hover:bg-fg/10 hover:text-fg";
  return (
    <button
      type="button"
      onClick={onClick}
      title={title}
      aria-label={title}
      className={`${BUTTON} flex items-center justify-center text-fg-subtle transition-colors ${hover}`}
    >
      {children}
    </button>
  );
}

/** 12×12 stroked glyph on a half-pixel grid so the 1px strokes stay crisp. */
function Glyph({ d }: { d: string }) {
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
      <path d={d} stroke="currentColor" strokeWidth="1" />
    </svg>
  );
}
