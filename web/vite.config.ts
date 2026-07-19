import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    proxy: {
      // Dev: the Rust API runs on :8080; the built app is served by it directly.
      "/api": "http://localhost:8080",
    },
  },
});
