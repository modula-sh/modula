import { Channel } from "@tauri-apps/api/core";
import { FitAddon } from "@xterm/addon-fit";
import { type ITheme, Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { useEffect, useRef, useState } from "react";
import { useThemeContext } from "../../contexts/ThemeContext";
import type { ThemeMode } from "../../hooks/useTheme";
import { client, errorMessage } from "../../services/client";
import { Button } from "../Button";

/** localStorage flag: open new conversations in terminal mode. */
export const TERMINAL_MODE_KEY = "modula.conversations.terminal";

const FONT = '"IBM Plex Mono", ui-monospace, Menlo, monospace';

// Backgrounds match --color-bg. TUIs that query it (OSC 11) do so at launch, so a
// running session keeps the palette it detected when the app theme flips.
const THEMES: Record<ThemeMode, ITheme> = {
  dark: {
    background: "#101012",
    foreground: "#f4f4f5",
    cursor: "#f4f4f5",
    selectionBackground: "#3f3f46",
  },
  // xterm's default ANSI colors are tuned for dark backgrounds.
  light: {
    background: "#fafafb",
    foreground: "#18181b",
    cursor: "#18181b",
    selectionBackground: "#d4d4d8",
    black: "#24292f",
    red: "#cf222e",
    green: "#116329",
    yellow: "#4d2d00",
    blue: "#0969da",
    magenta: "#8250df",
    cyan: "#1b7c83",
    white: "#6e7781",
    brightBlack: "#57606a",
    brightRed: "#a40e26",
    brightGreen: "#1a7f37",
    brightYellow: "#633c01",
    brightBlue: "#218bff",
    brightMagenta: "#a475f9",
    brightCyan: "#3192aa",
    brightWhite: "#8c959f",
  },
};

/** The provider's own TUI, running in an engine-hosted PTY. Unmounting detaches
 * the view; the process keeps running until it exits or the engine stops. */
export function TerminalView({
  ws,
  conversationId,
  initialPrompt,
}: {
  ws: string;
  conversationId: string;
  /** Sent with the first launch only; the engine ignores it once a session exists. */
  initialPrompt?: string;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const { mode } = useThemeContext();
  const theme = THEMES[mode];
  const themeRef = useRef(theme);
  themeRef.current = theme;
  const [exitCode, setExitCode] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Bumped by Resume to start a fresh view, which relaunches the session.
  const [launch, setLaunch] = useState(0);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    setExitCode(null);
    setError(null);
    const term = new Terminal({
      fontFamily: FONT,
      fontSize: 12,
      theme: themeRef.current,
      cursorBlink: true,
    });
    termRef.current = term;
    const fit = new FitAddon();
    term.loadAddon(fit);
    // A hidden view (another workspace is showing) measures 0×0.
    const fitIfShown = () => {
      if (el.clientWidth > 0 && el.clientHeight > 0) fit.fit();
    };

    const onData = new Channel<ArrayBuffer>();
    onData.onmessage = (buf) => term.write(new Uint8Array(buf));
    const onExit = new Channel<number>();
    onExit.onmessage = setExitCode;

    let opened = false;
    let disposed = false;
    // Measure cells with the real font, not its fallback.
    document.fonts.load(`12px ${FONT}`).finally(() => {
      if (disposed) return;
      term.open(el);
      fitIfShown();
      // A paired phone shares this PTY and may have resized it; focus takes it back.
      term.textarea?.addEventListener("focus", () => {
        if (opened)
          client.terminal.resize(ws, conversationId, term.cols, term.rows).catch(() => {});
      });
      term.focus();
      const { cols, rows } = term;
      client.terminal
        .open(ws, conversationId, initialPrompt, cols, rows, onData, onExit)
        .then(() => {
          opened = true;
          if (term.cols !== cols || term.rows !== rows) {
            client.terminal.resize(ws, conversationId, term.cols, term.rows).catch(() => {});
          }
        })
        .catch((e) => setError(errorMessage(e)));
    });

    term.onData((data) => {
      client.terminal.write(ws, conversationId, data).catch(() => {});
    });
    term.onResize(({ cols, rows }) => {
      if (opened) client.terminal.resize(ws, conversationId, cols, rows).catch(() => {});
    });
    // xterm sends a bare CR for Shift+Enter; ESC CR is the TUIs' newline.
    term.attachCustomKeyEventHandler((e) => {
      if (e.key !== "Enter" || !e.shiftKey) return true;
      if (e.type === "keydown") client.terminal.write(ws, conversationId, "\x1b\r").catch(() => {});
      return false;
    });
    const observer = new ResizeObserver(fitIfShown);
    observer.observe(el);

    return () => {
      disposed = true;
      onData.onmessage = () => {};
      onExit.onmessage = () => {};
      observer.disconnect();
      term.dispose();
      termRef.current = null;
    };
  }, [ws, conversationId, initialPrompt, launch]);

  useEffect(() => {
    if (termRef.current) termRef.current.options.theme = theme;
  }, [theme]);

  return (
    <div
      className="flex-1 flex flex-col overflow-hidden min-w-0"
      style={{ background: theme.background }}
    >
      {/* FitAddon sizes to the container's box, so padding lives on a wrapper. */}
      <div className="flex-1 min-h-0 flex px-3 pt-2">
        <div ref={containerRef} className="flex-1 min-w-0" />
      </div>
      {(exitCode !== null || error) && (
        <div className="flex items-center gap-3 px-4 py-2 border-t border-border text-xs text-fg-subtle">
          <span className={error ? "text-red-400" : undefined}>
            {error ?? `Session ended (exit ${exitCode}).`}
          </span>
          <Button onClick={() => setLaunch((n) => n + 1)}>Resume</Button>
        </div>
      )}
    </div>
  );
}
