import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import TOML from "toml";
import { defineConfig } from "vite";

const version_number: string = TOML.parse(
  readFileSync(resolve(import.meta.dirname, "../Cargo.toml"), "utf-8"),
)["package"]["version"];

// https://vite.dev/config/
export default defineConfig({
  define: {
    __APHANITE_VERSION: JSON.stringify(version_number),
  },
  plugins: [tailwindcss(), svelte()],
  base: "./",
  server: {
    proxy: {
      "/api": "http://localhost:3000",
    },
  },
  build: {
    assetsDir: "_assets",
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, "index.html"),
        login: resolve(import.meta.dirname, "login.html"),
        "not-found": resolve(import.meta.dirname, "not-found.html"),
      },
    },
  },
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
});
