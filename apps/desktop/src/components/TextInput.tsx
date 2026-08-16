/** Standard text input. Accepts every native <input> attribute (value, onChange,
 * placeholder, type, autoFocus, …) plus a `mono` shorthand for `font-mono`,
 * `padded` (default false) for the larger touch target, and `className` for any
 * one-off layout adjustment (e.g. width). */
export function TextInput({
  mono,
  padded = false,
  className = "",
  ...props
}: Omit<React.InputHTMLAttributes<HTMLInputElement>, "className"> & {
  mono?: boolean;
  padded?: boolean;
  className?: string;
}) {
  const base =
    "bg-surface border border-border rounded text-xs text-fg placeholder-fg-subtle focus:outline-none focus:border-border-focus disabled:opacity-60 disabled:cursor-not-allowed";
  const padding = padded ? "px-2 py-1.5" : "px-2 py-1";
  const font = mono ? " font-mono" : "";
  return <input {...props} className={`${base} ${padding}${font} ${className}`.trim()} />;
}
