/** Wide rectangular call-to-action — onboarding / landing flows only. */
export function LargeButton({
  children,
  className = "",
  type = "button",
  ...props
}: Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "className"> & {
  className?: string;
}) {
  const style =
    "inline-flex items-center justify-center px-8 py-2.5 min-w-[16rem] rounded-md bg-surface border border-border text-sm text-fg-muted font-inter transition-colors hover:text-fg hover:bg-surface-2 hover:border-border-focus/20 disabled:opacity-50 disabled:cursor-not-allowed";
  return (
    <button {...props} type={type} className={`${style} ${className}`.trim()}>
      {children}
    </button>
  );
}
