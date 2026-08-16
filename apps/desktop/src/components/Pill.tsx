import type { PipelineTone } from "../types";

const TONE_BG: Record<string, string> = {
  zinc: "bg-surface-2 text-fg",
  green: "bg-green-100 text-green-800 dark:bg-green-950 dark:text-green-300",
  yellow: "bg-yellow-100 text-yellow-800 dark:bg-yellow-950 dark:text-yellow-300",
  red: "bg-red-100 text-red-800 dark:bg-red-950 dark:text-red-300",
  blue: "bg-blue-100 text-blue-800 dark:bg-blue-950 dark:text-blue-300",
  purple: "bg-purple-100 text-purple-800 dark:bg-purple-950 dark:text-purple-300",
  orange: "bg-orange-100 text-orange-800 dark:bg-orange-950 dark:text-orange-300",
};

const TONE_BORDER: Record<string, string> = {
  zinc: "border-border",
  green: "border-green-200 dark:border-green-900",
  yellow: "border-yellow-200 dark:border-yellow-900",
  red: "border-red-200 dark:border-red-900",
  blue: "border-blue-200 dark:border-blue-900",
  purple: "border-purple-200 dark:border-purple-900",
  orange: "border-orange-200 dark:border-orange-900",
};

export function Pill({
  children,
  tone = "zinc",
  size = "md",
  variant = "bordered",
  title,
  className = "",
}: {
  children: React.ReactNode;
  tone?: PipelineTone;
  /** `sm` for slim row contexts (run lists, dense tables). */
  size?: "sm" | "md";
  /** `flat` drops the border + uppercase styling and uses pill-rounded ends. */
  variant?: "bordered" | "flat";
  /** Native tooltip — e.g. to expose a full id when the pill shows a label. */
  title?: string;
  className?: string;
}) {
  const sizing = size === "sm" ? "px-1.5 py-0 text-[10px]" : "px-[8.5px] py-[4.5px] text-xs";
  const shape =
    variant === "flat"
      ? "rounded-full"
      : `rounded-full border ${TONE_BORDER[tone]} uppercase tracking-wide`;
  return (
    <span
      title={title}
      className={`inline-flex items-center gap-1 ${sizing} ${shape} ${TONE_BG[tone]} whitespace-nowrap shrink-0 ${className}`}
    >
      {children}
    </span>
  );
}
