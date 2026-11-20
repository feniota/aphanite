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
  overrides: [
    {
      // node:test returns a promise that the test runner owns.
      files: ["tests/js/**/*.ts"],
      rules: { "typescript/no-floating-promises": "off" },
    },
  ],
});
