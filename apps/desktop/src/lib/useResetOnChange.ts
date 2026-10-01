import { useState } from "react";

const UNSET = Symbol("unset");

/** Runs `reset` during render on mount and whenever `key` changes; `reset` may
 * only set the caller's own state. An effect would also re-run when a hidden
 * workspace is shown again and wipe what the user left there. */
export function useResetOnChange(key: unknown, reset: () => void) {
  const [prev, setPrev] = useState<unknown>(UNSET);
  if (!Object.is(prev, key)) {
    setPrev(key);
    reset();
  }
}
