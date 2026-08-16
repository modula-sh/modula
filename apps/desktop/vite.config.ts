import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { version } from "./package.json";

const FRONTEND_PORT = parseInt(process.env.MODULA_FRONTEND_PORT ?? "9100", 10);

export default defineConfig({
  define: { __APP_VERSION__: JSON.stringify(version) },
  plugins: [react()],
  server: {
    port: FRONTEND_PORT,
    strictPort: true,
  },
});
