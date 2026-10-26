import path from "node:path";

import js from "@eslint/js";
import oxlint from "eslint-plugin-oxlint";
import svelte from "eslint-plugin-svelte";
import { defineConfig, globalIgnores, includeIgnoreFile } from "eslint/config";
import globals from "globals";
import ts from "typescript-eslint";

const gitignorePath = path.resolve(import.meta.dirname, ".gitignore");

export default defineConfig(
  includeIgnoreFile(gitignorePath),
  globalIgnores([
    "./web/dist/**/*",
    "./src/**/*",
    "target/**/*",
    ".github/**/*",
    ".forgejo/**/*",
    "./scripts/**/*",
  ]),
  js.configs.recommended,
  ts.configs.recommended,
  svelte.configs.recommended,
  {
    languageOptions: { globals: { ...globals.browser, ...globals.node } },
    rules: {
      // typescript-eslint strongly recommend that you do not use the no-undef lint rule on TypeScript projects.
      // see: https://typescript-eslint.io/troubleshooting/faqs/eslint/#i-get-errors-from-the-no-undef-rule-about-global-variables-not-being-defined-even-though-there-are-no-typescript-errors
      "no-undef": "off",
    },
  },
  {
    files: ["**/*.svelte", "**/*.svelte.ts", "**/*.svelte.js"],
    languageOptions: {
      parserOptions: {
        projectService: true,
        extraFileExtensions: [".svelte"],
        parser: ts.parser,
      },
    },
  },
  {
    // Override or add rule settings here, such as:
    // 'svelte/button-has-type': 'error'
    rules: {},
  },
  {
    files: ["./web/src/lib/index.ts", "./web/src/lib/components/ui/**/*"],
    rules: {
      "@typescript-eslint/no-explicit-any": "off",
    },
  },
  {
    // Ambient global declarations must use `var` to land on `typeof globalThis`.
    files: ["**/*.d.ts"],
    rules: {
      "no-var": "off",
    },
  },
  ...(await oxlint.buildFromConfigFile("./oxlint.config.ts")),
);
