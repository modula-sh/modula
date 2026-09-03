import { createContext, useContext, useEffect, useRef } from "react";
import { isMac } from "../tauri/window";

/** A chord is `[mod+][shift+][alt+]<key>`, e.g. `mod+k` or `mod+shift+p`.
 * `mod` is ⌘ on macOS and Ctrl on Windows/Linux. */
type Spec = { mod: boolean; shift: boolean; alt: boolean; key: string };

type Entry = { spec: Spec; run: (e: KeyboardEvent) => void };

const ShortcutsContext = createContext<Entry[]>([]);

function parse(chord: string): Spec {
  const parts = chord.toLowerCase().split("+");
  const key = parts.pop() ?? "";
  return {
    mod: parts.includes("mod"),
    shift: parts.includes("shift"),
    alt: parts.includes("alt"),
    key,
  };
}

/** The physical key rather than the character it produces, so AZERTY, Cyrillic
 * and caps lock all still match the letter printed on a US keyboard. */
function pressedKey(e: KeyboardEvent): string {
  if (e.code.startsWith("Key")) return e.code.slice(3).toLowerCase();
  if (e.code.startsWith("Digit")) return e.code.slice(5);
  return e.key.toLowerCase();
}

function matches(spec: Spec, e: KeyboardEvent, mac: boolean): boolean {
  // The other platform's modifier has to be up: Win+K opens a system panel on
  // Windows, and ⌃K is kill-to-end-of-line in every macOS text field.
  const mod = mac ? e.metaKey : e.ctrlKey;
  const otherMod = mac ? e.ctrlKey : e.metaKey;
  return (
    mod === spec.mod &&
    !otherMod &&
    e.shiftKey === spec.shift &&
    e.altKey === spec.alt &&
    pressedKey(e) === spec.key
  );
}

function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  return !!el && (el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName));
}

/** One window keydown listener for the whole app. The most recently mounted
 * match wins, so a view can shadow a layout-level binding. */
export function ShortcutsProvider({ children }: { children: React.ReactNode }) {
  const entries = useRef<Entry[]>([]).current;

  useEffect(() => {
    const mac = isMac();
    function onKey(e: KeyboardEvent) {
      for (let i = entries.length - 1; i >= 0; i--) {
        const { spec, run } = entries[i];
        // An unmodified key belongs to whatever the user is typing into.
        if (!spec.mod && !spec.alt && isTyping(e.target)) continue;
        if (!matches(spec, e, mac)) continue;
        e.preventDefault();
        run(e);
        return;
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [entries]);

  return <ShortcutsContext.Provider value={entries}>{children}</ShortcutsContext.Provider>;
}

/** Bind `chord` for as long as the calling component is mounted. */
export function useShortcut(chord: string, handler: (e: KeyboardEvent) => void) {
  const entries = useContext(ShortcutsContext);
  const latest = useRef(handler);
  latest.current = handler;

  useEffect(() => {
    const entry: Entry = { spec: parse(chord), run: (e) => latest.current(e) };
    entries.push(entry);
    return () => {
      entries.splice(entries.indexOf(entry), 1);
    };
  }, [entries, chord]);
}
