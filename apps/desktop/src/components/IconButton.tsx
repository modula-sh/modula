/** Borderless icon button for chrome controls — round hover bg, subtle fg. */
export function IconButton({
  children,
  className = "",
  type = "button",
  ...props
}: Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "className"> & {
  className?: string;
}) {
  const style =
    "shrink-0 p-1.5 rounded-full text-fg-subtle hover:text-fg hover:bg-fg/10 transition-colors disabled:opacity-30 disabled:pointer-events-none";
  return (
    <button {...props} type={type} className={`${style} ${className}`.trim()}>
      {children}
    </button>
  );
}
