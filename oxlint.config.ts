import { defineConfig } from "oxlint";

export default defineConfig({
  plugins: ["import"],
  rules: {
    "import/extensions": ["deny", "ignorePackages"],
  },
  options: {
    typeAware: true,
    typeCheck: true,
  },
  ignorePatterns: ["./web/dist/**/*", "./src/**/*", "target/**/*"],
});
