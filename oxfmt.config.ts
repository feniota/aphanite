import { defineConfig } from "oxfmt";

export default defineConfig({
  bracketSameLine: true,
  svelte: true,
  sortImports: true,
  sortTailwindcss: true,
  sortPackageJson: true,
  ignorePatterns:["**/*.md"]
});
