export function OpenCodeIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 30 30"
      fill="currentColor"
      fillRule="evenodd"
      preserveAspectRatio="xMidYMid meet"
      className={className}
      aria-label="OpenCode"
    >
      <title>OpenCode</title>
      <path d="M3 0h24v30H3V0Zm6 6v18h12V6H9Z" />
      <path d="M9 12h12v12H9z" fillOpacity="0.55" />
    </svg>
  );
}
