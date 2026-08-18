/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: ["selector", '[data-theme="dark"]'],
  theme: {
    extend: {
      borderWidth: {
        DEFAULT: "0.5px",
      },
      boxShadow: {
        panel: "var(--shadow-panel)",
        content: "var(--shadow-content)",
        card: "var(--shadow-card)",
        popover: "var(--shadow-popover)",
      },
      colors: {
        bg: "rgb(var(--color-bg) / <alpha-value>)",
        chrome: "rgb(var(--color-chrome) / <alpha-value>)",
        surface: "rgb(var(--color-surface) / <alpha-value>)",
        "surface-2": "rgb(var(--color-surface-2) / <alpha-value>)",
        card: "rgb(var(--color-card) / <alpha-value>)",
        "card-border": "rgb(var(--color-card-border) / <alpha-value>)",
        "chat-input": "rgb(var(--color-chat-input) / <alpha-value>)",
        "chat-input-border": "rgb(var(--color-chat-input-border) / <alpha-value>)",
        border: "rgb(var(--color-border) / <alpha-value>)",
        "border-focus": "rgb(var(--color-border-focus) / <alpha-value>)",
        edge: "rgb(var(--color-edge) / <alpha-value>)",
        fg: "rgb(var(--color-fg) / <alpha-value>)",
        "fg-muted": "rgb(var(--color-fg-muted) / <alpha-value>)",
        "fg-subtle": "rgb(var(--color-fg-subtle) / <alpha-value>)",
        overlay: "var(--color-overlay)",
      },
      fontFamily: {
        inter: ['"Inter Variable"', "Inter", "ui-sans-serif", "system-ui", "sans-serif"],
        hanken: [
          '"Hanken Grotesk Variable"',
          '"Hanken Grotesk"',
          "ui-sans-serif",
          "system-ui",
          "sans-serif",
        ],
        mono: [
          '"IBM Plex Mono"',
          "ui-monospace",
          "SFMono-Regular",
          "Menlo",
          "Monaco",
          "Consolas",
          "monospace",
        ],
      },
    },
  },
  plugins: [],
};
