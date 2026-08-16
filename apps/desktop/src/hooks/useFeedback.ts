import { useEffect, useRef, useState } from "react";

export interface Feedback {
  kind: "ok" | "err";
  text: string;
}

/** Transient feedback state for forms — the inline "saved" / "error" line
 * after a Save/Delete/Run action.
 *
 * Centralises the timer-clear pattern so call sites don't sprinkle
 * `setTimeout(setFeedback, 5000)` everywhere. The timer is cleared when
 * a new feedback is set (so two errors in a row don't race) and on unmount.
 *
 * Usage:
 *   const fb = useFeedback();
 *   try { ...; fb.ok('saved'); }
 *   catch (e) { fb.err(errorMessage(e)); }
 *
 *   // optional auto-clear:
 *   fb.ok('spawned · pid 123', { clearAfter: 5000 });
 *
 *   // render:
 *   <FeedbackText feedback={fb.feedback} />
 */
export function useFeedback() {
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  function cancelTimer() {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }

  function set(next: Feedback | null, opts?: { clearAfter?: number }) {
    cancelTimer();
    setFeedback(next);
    if (next && opts?.clearAfter) {
      timerRef.current = setTimeout(() => setFeedback(null), opts.clearAfter);
    }
  }

  // Cleanup on unmount.
  useEffect(() => () => cancelTimer(), []);

  return {
    feedback,
    ok: (text: string, opts?: { clearAfter?: number }) => set({ kind: "ok", text }, opts),
    err: (text: string, opts?: { clearAfter?: number }) => set({ kind: "err", text }, opts),
    clear: () => set(null),
  };
}
