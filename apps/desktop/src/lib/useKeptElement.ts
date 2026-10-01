import { useCallback, useState } from "react";

/** A portal target held in state, as `[el, ref]`. Hiding an `<Activity>` detaches
 * refs but keeps the node, so the target is cleared only once the node is gone. */
export function useKeptElement<T extends HTMLElement>() {
  const [el, setEl] = useState<T | null>(null);
  const ref = useCallback((node: T | null) => {
    if (node) setEl(node);
    // Deferred: React detaches before it removes the node from the DOM.
    else queueMicrotask(() => setEl((prev) => (prev?.isConnected ? prev : null)));
  }, []);
  return [el, ref] as const;
}
