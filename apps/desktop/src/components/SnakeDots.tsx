// Six 3×3px dots on a 2×3 grid. Negative delays start each dot mid-cycle so a
// single bright dot snakes clockwise around the perimeter, trailing a fade.
// Delays are in DOM (row-major) order: TL, TR, ML, MR, BL, BR.
const DELAYS = ["0s", "-0.15s", "-0.75s", "-0.3s", "-0.6s", "-0.45s"];

export function SnakeDots({ className = "" }: { className?: string }) {
  return (
    <span
      role="status"
      aria-label="loading"
      className={`grid grid-cols-[3px_3px] grid-rows-[3px_3px_3px] gap-[2px] ${className}`.trim()}
    >
      {DELAYS.map((delay) => (
        <span key={delay} className="snake-dot bg-current" style={{ animationDelay: delay }} />
      ))}
    </span>
  );
}
