import { defineConfig } from "oxfmt";

export default defineConfig({
  arrowParens: "avoid",
  bracketSameLine: true,
  svelte: true,
  sortImports: true,
  sortTailwindcss: true,
  sortPackageJson: true,
  ignorePatterns: ["**/*.md", "target/**/*"],
  overrides: [
    {
      files: ["./deno.jsonc"],
      options: {
        trailingComma: "none",
      },
    },
  ],
});
