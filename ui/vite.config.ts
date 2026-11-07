import { defineConfig } from "vitest/config";

export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "safari15", outDir: "dist", emptyOutDir: true },
  test: { environment: "jsdom" },
});
