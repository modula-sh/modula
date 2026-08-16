/** Borderless secondary action sized to match LargeButton — pair them in a row. */
export function BackButton({
  children,
  className = "",
  type = "button",
  ...props
}: Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "className"> & {
  className?: string;
}) {
  const style =
    "inline-flex items-center justify-center px-8 py-2.5 min-w-[16rem] rounded-md border border-border text-sm text-fg-muted font-inter transition-colors hover:text-fg hover:border-border-focus/20 disabled:opacity-50 disabled:cursor-not-allowed";
  return (
    <button {...props} type={type} className={`${style} ${className}`.trim()}>
      {children}
    </button>
  );
}
