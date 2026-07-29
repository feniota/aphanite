# aphanite/web

Here is the frontend part of Aphanite, written in Svelte and TypeScript, built with Vite.

## Naming Conventions

Aphanite's main developers are Rustaceans, so identifiers' cases here are enforced to pair with Rust rules, that is:

- For local variables and functions (including methods), use `snake_case`.
- For constants (that are static, or not dymanically created) or global variables, use `SCREAMING_SNAKE_CASE`.
- For `type`s, `interface`s, `class`es, `enum`s, enum variants, or UI components, use `PascalCase`.
- For pure TypeScript, CSS or pure HTML files, use `kebab-case`.
- For Svelte files, use `PascalCase`.
- `camelCase` should generally be avoided.
- Though, imported foreign identifiers should NOT be renamed to match these conventions.
