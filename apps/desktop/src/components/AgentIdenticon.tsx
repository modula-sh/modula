import { useEffect, useRef } from "react";
import { useThemeContext } from "../contexts/ThemeContext";
import { generateIdenticon, IDENTICON_SIZE } from "../lib/identicon";

export function AgentIdenticon({
  id,
  size = 28,
  className = "",
}: {
  id: string;
  size?: number;
  className?: string;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  const { theme } = useThemeContext();
  useEffect(() => {
    // Defer to the next frame so the read happens after ThemeProvider's effect
    // has updated <html data-theme>; otherwise we'd sample the previous theme's
    // color (child effects flush before parent effects).
    const frame = requestAnimationFrame(() => {
      const canvas = ref.current;
      const ctx = canvas?.getContext("2d");
      if (!canvas || !ctx) return;
      // Paint at native 7×7 resolution; CSS upscales with nearest-neighbor
      // (image-rendering: pixelated) so each cell is a true, crisp pixel block.
      const grid = generateIdenticon(id);
      ctx.clearRect(0, 0, IDENTICON_SIZE, IDENTICON_SIZE);
      ctx.fillStyle = getComputedStyle(canvas).color; // resolve currentColor for theming
      grid.forEach((row, r) =>
        row.forEach((v, c) => {
          if (v <= 0) return;
          ctx.globalAlpha = v;
          ctx.fillRect(c, r, 1, 1);
        }),
      );
    });
    return () => cancelAnimationFrame(frame);
  }, [id, theme]);
  return (
    <canvas
      ref={ref}
      width={IDENTICON_SIZE}
      height={IDENTICON_SIZE}
      style={{ width: size, height: size, imageRendering: "pixelated" }}
      aria-hidden
      className={`block ${className}`}
    />
  );
}
