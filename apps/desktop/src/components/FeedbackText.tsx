import type { Feedback } from "../hooks/useFeedback";

/** Inline transient feedback line — the "saved" / "spawned · pid 123" /
 * "validation error" text that appears after a form action. */
export function FeedbackText({ feedback }: { feedback: Feedback | null }) {
  if (!feedback) return null;
  return (
    <span
      className={
        feedback.kind === "ok" ? "text-[11px] text-green-400 ml-1" : "text-[11px] text-red-400 ml-1"
      }
    >
      {feedback.text}
    </span>
  );
}
