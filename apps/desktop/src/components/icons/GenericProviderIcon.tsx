export function GenericProviderIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-label="Provider"
    >
      <title>Provider</title>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <polyline points="7,10 10,13 7,16" />
      <line x1="13" y1="16" x2="17" y2="16" />
    </svg>
  );
}
