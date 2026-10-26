import { defineConfig } from "oxlint";

export default defineConfig({
  plugins: ["import", "eslint", "typescript", "oxc", "promise"],
  rules: {
    "import/extensions": ["deny", "ignorePackages"],
  },
  options: {
    typeAware: true,
    typeCheck: true,
  },
  ignorePatterns: ["./web/dist/**/*", "target/**/*", ".github/**/*", ".forgejo/**/*"],
});
