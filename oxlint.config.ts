import { defineConfig } from "oxlint";

export default defineConfig({
  options: {
    typeAware: true,
    typeCheck: true,
  },
  ignorePatterns: ["./web/dist/**/*", "./src/**/*", "target/**/*"],
});
