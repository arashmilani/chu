import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src-fe/tests/setup.ts"],
    include: ["src-fe/**/*.{test,spec}.{ts,tsx}"],
    pool: "threads",
  },
});
