import { defineConfig } from "vitest/config";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  clearScreen: false,
  plugins: [tailwindcss()],
  server: { port: 5173, strictPort: true },
  build: {
    target: "safari15",
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: { input: { main: "index.html", editor: "editor.html" } },
  },
  oxc: { jsx: { runtime: "automatic", importSource: "preact" } },
  test: { environment: "jsdom" },
});
