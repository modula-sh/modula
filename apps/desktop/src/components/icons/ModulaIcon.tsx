import { useId } from "react";

// Modula brandmark — interlocking "module" blocks rising from the bottom-left
// to the top-right, matching the app icon. Path sourced from the official asset.
// Gradient-styled to match the marketing site header: a diagonal fade of
// currentColor from full opacity down to 0.35, so it still adapts to theme.
export function ModulaIcon({ className }: { className?: string }) {
  const gradientId = useId();
  return (
    <svg viewBox="0 0 370 370" fill="none" className={className} aria-label="Modula">
      <title>Modula</title>
      <defs>
        <linearGradient
          id={gradientId}
          x1="0"
          y1="0"
          x2="370"
          y2="370"
          gradientUnits="userSpaceOnUse"
        >
          <stop offset="0%" stopColor="currentColor" stopOpacity="1" />
          <stop offset="42%" stopColor="currentColor" stopOpacity="1" />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0.35" />
        </linearGradient>
      </defs>
      <path
        d="M322 0C348.51 0 370 21.4903 370 48V102C370 128.51 348.51 150 322 150H268C263.73 150 260 153.73 260 158V212C260 216.269 263.731 220 268 220H322C348.51 220 370 241.49 370 268V322C370 348.51 348.51 370 322 370H268C241.49 370 220 348.51 220 322V268C220 263.731 216.269 260 212 260H158C153.73 260 150 263.73 150 268V322C150 348.51 128.51 370 102 370H48C21.4903 370 0 348.51 0 322V268C0 241.49 21.4903 220 48 220H102C106.269 220 110 216.269 110 212V158C110 131.49 131.49 110 158 110H212C216.269 110 220 106.269 220 102V48C220 21.4903 241.49 0 268 0H322Z"
        fill={`url(#${gradientId})`}
      />
    </svg>
  );
}
