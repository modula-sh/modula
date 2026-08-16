// Deterministic 7×7 identicon with horizontal symmetry. Cells hold the rendered
// opacity (0, 0.4, or 1) so the renderer can stay dumb.
export const IDENTICON_SIZE = 7;

export function generateIdenticon(id: string): number[][] {
  const rng = mulberry32(hashSeed(id || "default"));
  const grid: number[][] = Array.from({ length: IDENTICON_SIZE }, () =>
    Array(IDENTICON_SIZE).fill(0),
  );
  const half = Math.ceil(IDENTICON_SIZE / 2);
  for (let r = 0; r < IDENTICON_SIZE; r++) {
    for (let c = 0; c < half; c++) {
      const x = rng();
      const v = x < 0.42 ? 0 : x < 0.72 ? 0.4 : 1;
      grid[r][c] = v;
      grid[r][IDENTICON_SIZE - 1 - c] = v;
    }
  }
  return grid;
}

function hashSeed(s: string): number {
  let h = 2166136261 >>> 0;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

function mulberry32(seed: number) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
