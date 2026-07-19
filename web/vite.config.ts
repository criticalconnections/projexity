import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Configurable dev proxy target (no @types/node dependency, so read
// process.env through globalThis with a narrow cast).
const devApi =
  (
    globalThis as unknown as {
      process?: { env?: Record<string, string | undefined> };
    }
  ).process?.env?.PJX_DEV_API ?? "http://localhost:8080";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    proxy: {
      // Dev: the Rust API runs on :8080 by default (override with
      // PJX_DEV_API); the built app is served by it directly.
      "/api": devApi,
    },
  },
});
