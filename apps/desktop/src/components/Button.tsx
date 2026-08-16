/** Standard action button — pill with subtle border. Tones:
 *   - neutral (default) — the pill; used for create/save/run/delete/cancel.
 *   - link              — borderless gray text, for inline subtle actions
 *                         (close ✕, "+ add row", back arrows).
 *   - tab               — active-togglable pill for nav tabs. */
export function Button({
  children,
  tone = "neutral",
  active,
  className = "",
  type = "button",
  ...props
}: Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "className"> & {
  tone?: "neutral" | "link" | "tab";
  /** Only meaningful for `tone="tab"`; toggles the selected style. */
  active?: boolean;
  className?: string;
}) {
  let style: string;
  if (tone === "tab") {
    style =
      "px-2.5 py-1 rounded text-xs uppercase tracking-wide transition-colors disabled:opacity-50 disabled:cursor-not-allowed " +
      (active ? "bg-surface-2 text-fg" : "text-fg-subtle hover:text-fg hover:bg-surface");
  } else if (tone === "link") {
    style = "text-xs text-fg-muted hover:text-fg disabled:opacity-50 disabled:cursor-not-allowed";
  } else {
    style =
      "inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-surface border border-border text-xs text-fg-muted font-inter transition-colors hover:text-fg hover:bg-surface-2 hover:border-border-focus/20 disabled:opacity-50 disabled:cursor-not-allowed";
  }
  return (
    <button {...props} type={type} className={`${style} ${className}`.trim()}>
      {children}
    </button>
  );
}
