# Aphanite AGENTS.md

## Project overview

This is a monorepo containing:

- A **Rust backend** (`src/`) — [ntex](https://ntex.rs) web server, Yggdrasil Minecraft auth + custom management API + Phenocryst TOTP
- A **Svelte frontend** (`web/`) — Vite-built, three-entry-page SPA (management panel, login/register, not-found)
- **Node.js TypeScript tooling** — Vite config, ESLint/Oxlint/Oxfmt configs, black-box tests (`tests/js/`), and helper scripts (`scripts/`)

**pnpm is the preferred package manager.** Use `pnpm <script>` instead of `npm run`. Scripts run on Node.js (v24+) using its native TypeScript support.
The `justfile` is the cross-language task runner (`just dev`, `just build`, `just test`, `just lint`, `just format`).

### Documentation

Project documentation lives on a dedicated VitePress site:

| Language | URL                                               |
| -------- | ------------------------------------------------- |
| English  | `https://mc.feniota.org/aphanite/development/`    |
| Chinese  | `https://mc.feniota.org/zh/aphanite/development/` |

Key pages:

- **General API**: `https://mc.feniota.org/aphanite/development/Aphanite%20General.md`
- **Yggdrasil API**: `https://mc.feniota.org/aphanite/development/Yggdrasil.md` (mostly refers to external Yggdrasil spec docs)

The Markdown source files are at `https://git.feniota.org/feniota/mc-docs.git`. The canonical repository is Forgejo (`git.feniota.org`); GitHub is a mirror kept in sync by CI scripts.

---

## Build, test, and lint

### Backend (Rust)

```bash
cargo build                  # Build the project
cargo build --release        # Build with optimizations (embeds web/dist)
cargo test                   # Unit tests + integration tests in tests/rust/
cargo test -- --nocapture    # Run tests with stdout visible
cargo clippy                 # Lint the codebase
cargo clippy --no-deps -- -D warnings   # What CI enforces
cargo fmt                    # Format
cargo fmt -- --check         # Verify formatting
cargo doc --no-deps --open   # View local API docs
```

Run with:

```bash
cargo run                    # Dev server, default listen 127.0.0.1:3000
cargo run -- --config /path/to/config.toml
cargo run -- init            # Generate a default config.toml
cargo run -- --debug         # Enable debug logs (also --verbose for info)
cargo run -- --listen 0.0.0.0 --port 8080
```

Debug/dev build extras:

```bash
cargo run -- --with-test-user       # [debug only] Creates a test user on startup
cargo run -- --debug --with-test-user
```

Create an admin user (optionally with a nickname):

```bash
cargo run -- create-admin --email admin@example.com --password s3cret
cargo run -- create-admin --email admin@example.com --nickname Admin --password s3cret
```

There's also a `bacon.toml` with preconfigured watch-mode jobs (`bacon check`, `bacon run-long`, `bacon clippy-all`, …).

### Frontend (Svelte/TypeScript)

```bash
pnpm install                 # Install dependencies
pnpm check                   # svelte-check + tsc (web app, vite config, CI scripts)
pnpm lint                    # oxlint + eslint
pnpm lint:fix                # oxlint --fix + eslint --fix
pnpm build                   # Production build into web/dist/
pnpm test:js                 # Black-box HTTP tests (tests/js/*.test.ts)
pnpm long-cn                 # Reformat long cn() class lists, then pnpm check
```

Formatting is handled by **oxfmt** (not part of `pnpm lint`):

```bash
pnpm exec oxfmt              # Write formatting fixes
pnpm exec oxfmt --check      # Verify formatting (what CI enforces)
```

The Vite dev server can be started directly:

```bash
pnpm dev                     # = vite dev web/ (port 5173)
pnpm exec vite dev web/      # Same; `web/` is required because vite.config.ts is nested
```

Development flow: run `cargo run` and `pnpm dev` as two separate commands (AI agents should prefer this over `just dev` to keep logs separable). The debug backend proxies every non-API request to `http://localhost:5173`, while Vite proxies `/api` back to `http://localhost:3000`, so either port serves a working app. If the backend runs without Vite, UI routes return 502.

`web/vite.config.ts` defines three inputs (`index.html`, `login.html`, `not-found.html`), uses `base: "./"`, outputs assets to `_assets`, aliases `@` → `web/src`, and injects `__APHANITE_VERSION` read from `Cargo.toml`. A release build embeds `web/dist` into the binary via `rust-embed`, so **`pnpm build` must run before `cargo build --release`** (`just build` does both).

### Task runner (`justfile`)

| Recipe        | Purpose                                                                                                                                |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `just dev`    | Runs `dev-frontend` (`pnpm dev`) and `dev-backend` (`cargo run`) in parallel                                                           |
| `just build`  | `pnpm build`, then `cargo build --release`                                                                                             |
| `just test`   | `cargo test`, then `node --test "tests/js/*.test.ts"`                                                                                  |
| `just lint`   | Parallel frontend (`oxfmt --check`, `oxlint`, `eslint`, `pnpm check`) and backend (`cargo fmt --check`, `cargo check`, `cargo clippy`) |
| `just format` | Parallel `oxfmt` and `cargo fmt` (alias: `just fmt`)                                                                                   |

Sub-recipes such as `dev-frontend`, `dev-backend`, `build-frontend`, `lint-frontend`, `lint-backend`, `format-frontend`, `format-backend` are also available.

### Tests

- **Rust unit tests** — inline `#[cfg(test)]` modules (e.g. the token-bucket test in `src/kv_cache.rs`).
- **Rust integration tests** — `tests/rust/main.rs` declares `general_api`, `totp_api`, `yggdrasil_api`; `tests/rust/common/mod.rs` builds an in-process ntex test service with an in-memory Turso database. Run with `cargo test`.
- **JS black-box tests** — `tests/js/*.test.ts` spawn the real `target/debug/aphanite` binary and talk HTTP only. Build the binary first (`cargo build`), or set `APHANITE_BIN` to a specific binary path.

CI lives in `.forgejo/workflows/rust.yml` and `.forgejo/workflows/frontend.yml`; `.forgejo/scripts/` and `.github/scripts/` handle the Forgejo ↔ GitHub mirror and release syncing.

---

## High-level architecture

Aphanite is an open-source, self-deployable Yggdrasil (Minecraft auth server) with an optional Phenocryst (client instance management) extension. Built with **ntex** (web/runtime) + **toasty** (ORM, supports Turso and PostgreSQL).

The server exposes **two API surfaces**: the standard Yggdrasil authentication API (for authlib-injector clients) and a custom "General API" for user/profile management with its own token-based auth. Phenocryst contributes TOTP/verification endpoints.

### Backend module layout

```
src/
├── main.rs                      # Thin entrypoint: mimalloc, rustls CryptoProvider, calls aphanite::start()
├── lib.rs                       # start(): CLI/config, migrations, ORM, storage, AppState, HttpServer, cleanup task
├── cli.rs                       # Clap args (--debug/--verbose/--listen/--port/--config), tracing setup, early-exit commands
├── config.rs                    # TOML config parsing + RSA key loading (PKCS#8 PEM), config generation
├── types.rs                     # toasty models: User, Token, Instance, UserInstance, RegisterToken + Permission bitflags
├── data.rs                      # DatabaseAccessor: verify_user/token, create/delete tokens, profile queries, password update
├── data/
│   └── migrations.rs            # Custom migration runner (Turso / tokio-postgres), generated via build.rs
├── kv_cache.rs                  # In-memory KVCache (scc): login rate limiter, session join cache (30s), OTP sessions/tokens
├── storage.rs                   # AssetStorage: File model, Local/S3 abstraction, ref-counted dedup by BLAKE3 hash
├── assets/
│   └── config.example.toml      # Bundled default config (embedded via include_str!)
└── service/
    ├── mod.rs                   # Router root (/api, /api/yggdrasil, /assets), Error/ApiResponse, HttpState alias
    ├── api.rs                   # General API handlers (auth, users, profiles, registration, Turnstile)
    ├── types.rs                 # General API payloads: UserPayload, ProfilePayload
    ├── frontend.rs              # Debug: proxy to Vite :5173; release: serve embedded web/dist
    ├── json.rs                  # Custom JSON extractor mapping rejection status codes (400/413/415/422)
    ├── validation.rs            # validate_nickname / validate_profile_name / validate_password
    ├── yggdrasil/
    │   ├── mod.rs               # Yggdrasil route registration + 404 fallback
    │   ├── api.rs               # Yggdrasil handlers: auth, session, profile, texture upload/delete, meta
    │   └── types.rs             # GameProfile, ProfileTextures, textures payload, SkinModel, YggdrasilError, AphaniteClientIp
    └── phenocryst/
        ├── mod.rs               # Phenocryst module
        └── totp.rs              # TOTP create/delete + verification session endpoints
```

`AppState` (in `src/lib.rs`) carries `da` (`DatabaseAccessor`), `cfg` (`Arc<AppConfig>`), `assets` (`AssetStorage`), `kv` (`KVCache`), `rsa_pubkey`, `http_client` (`ntex::client::Client`), and `base_path` (clean subdirectory prefix derived from `service.path`).

Routing notes:

- `service::configure` mounts `/api` with the General API, TOTP endpoints, and `/api/yggdrasil`.
- `/assets` is served by `AssetStorage::configure`; it is mounted twice when `service.path` is set.
- The base-path scope sets `X-Authlib-Injector-API-Location` and falls back to `service::frontend::serve`.
- Debug builds proxy unknown routes to Vite; release builds resolve `web/dist` via `rust-embed`, fall back to `not-found.html`, support `Range`, and inject `window.__APHANITE_BASE` / `window.__APHANITE_NAME` into HTML.

### Frontend module layout

```
web/
├── index.html                   # Management panel entry
├── login.html                   # Login/register entry
├── not-found.html               # Standalone 404 entry
├── vite.config.ts               # Three inputs, @/ alias, /api proxy to :3000, version injection
├── svelte.config.js             # Filters out a11y warnings
├── public/                      # Static assets (auth image, fonts, favicon)
├── misc/capes.json              # Preset cape list shown in the skin upload dialog
└── src/
    ├── app.css                  # Tailwind v4 theme (glaucous palette) + fonts + global reset
    ├── App.svelte               # Management shell: TopBar + SideBar + router + Toast, AUTH validation
    ├── main.ts / login-main.ts / not-found-main.ts   # Entry mount points (all init i18n first)
    ├── definitions.d.ts         # Turnstile global + __APHANITE_VERSION declarations
    ├── locales/
    │   ├── index.ts             # i18next resource map
    │   ├── en/translation.json
    │   └── zh-CN/translation.json
    ├── lib/
    │   ├── api.ts               # REST functions + ApiResponse/ApiError + types; base from window.__APHANITE_BASE
    │   ├── auth.svelte.ts       # AuthState singleton AUTH — token/user/profiles, localStorage, validate()
    │   ├── i18n.svelte.ts       # i18next init, reactive t(), change_language()
    │   ├── home-page-router.svelte.ts # Route table shared by shell, sidebar and top bar
    │   ├── sidebar.svelte.ts    # Sidebar open/closed state (SIDEBAR)
    │   ├── darkmode.ts          # Light/dark/system handling (.dark on body, localStorage)
    │   ├── utils.ts             # cn(), Option<T>, transition_tick(), trim_*_matches(), CapeList
    │   ├── index.ts / lib.js    # shadcn-svelte compatibility barrel (re-exports cn)
    │   ├── Trans.svelte         # CJK–Latin typographic spacing for translated strings
    │   └── components/ui/       # shadcn-svelte components
    ├── components/
    │   ├── AuthImage.svelte     # Login background + credits overlay
    │   ├── DarkModeButton.svelte
    │   ├── Dialog.svelte        # Styled wrapper over Bits UI dialog
    │   ├── HomePageRouter.svelte / LoginPageRouter.svelte
    │   ├── HomePageSideBar.svelte / TopBar.svelte
    │   ├── LangSwitcher.svelte
    │   ├── MinecraftAvatar.svelte / MinecraftCape.svelte
    │   ├── Space.svelte         # 0.125ic spacer for CJK–Latin typography
    │   ├── Toast.svelte
    │   └── toast.svelte.ts      # Toast store; `toast(i18n_key, args?)`
    └── pages/
        ├── Login.svelte         # Multi-step login (email → password/TOTP)
        ├── Register.svelte      # Registration with Turnstile / register token
        ├── Home.svelte          # Dashboard: welcome, profile list, copy UUID
        ├── PlayerProfiles.svelte        # Profile list, create, batch delete
        ├── PlayerProfileDetails.svelte  # Skin/cape viewer + texture upload, rename, delete
        ├── User.svelte          # Account settings: profile info, password, TOTP
        ├── Management.svelte    # Admin-only user lookup/create and register links (rendered by User.svelte)
        └── NotFound.svelte      # Standalone 404 page
```

### shadcn-svelte (UI component library)

shadcn-svelte covers widgets that are tedious to style manually — select/dropdown menus, dialogs, checkboxes, tooltips. Everything else uses raw Tailwind v4. Only shadcn-generated code belongs in `web/src/lib/components/ui/`; project components go in `web/src/components/`.

Component source: `web/src/lib/components/ui/` (button, checkbox, dialog, dropdown-menu, select, separator, tooltip). Configuration: `components.json` (root) — `nova` style, `mist` base color, CSS variables enabled, Lucide icons.

Import patterns:

- `import { Button } from "@/lib/components/ui/button"` or `import { Checkbox } from "@/lib/components/ui/checkbox"` — for single-export components
- `import * as Select from "@/lib/components/ui/select"` — for multi-part components (`Select.Root`, `Select.Content`, `Select.Item`, …)
- `import * as DropdownMenu from "@/lib/components/ui/dropdown-menu"` — same pattern
- `import * as Tooltip from "@/lib/components/ui/tooltip"`
- `import * as BDialog from "@/lib/components/ui/dialog"` — raw Bits UI dialog primitives

`web/src/components/Dialog.svelte` wraps Bits UI with project styling (blur backdrop, custom close button, `no_default_styles` mode). It exposes imperative `open()`/`close()` methods via `bind:this`; use it for most modals. `App.svelte` provides `<TooltipProvider delayDuration={500}>` at the root.

CSS variables (`app.css`): shadcn-svelte compatibility variables (`--color-popover`, `--color-card`, `--color-secondary`, `--color-accent`, `--color-input`, `--color-destructive`) are layered on top of the custom glaucous palette, with light and `.dark` variants.

### Two API surfaces

1. **Yggdrasil API** (`/api/yggdrasil/`) — Minecraft authlib-injector server. Endpoints:
   - `GET ""` and `GET /` — metadata (server name, implementation, skin domains, signature public key)
   - `POST authserver/authenticate`, `refresh`, `validate`, `invalidate`, `signout`
   - `POST sessionserver/session/minecraft/join`
   - `GET sessionserver/session/minecraft/hasJoined`
   - `GET sessionserver/session/minecraft/profile/{uuid}`
   - `POST api/profiles/minecraft`
   - `PUT`/`DELETE api/user/profile/{uuid}/{texture_type}` (skin/cape upload and removal, max 8 MB)
   - Unknown Yggdrasil routes return a Yggdrasil-compatible 404 via `yggdrasil::fallback`.

   Uses `YggdrasilError` (Minecraft-compatible JSON error bodies). Rate-limited per username via `KVCache::try_consume` on `authenticate` and `signout`.

2. **General API** (`/api/`) — Custom Aphanite management API:
   - Auth: `POST auth/login` (password or `otp_token`), `POST auth/refresh`, `GET auth/validate`
   - Users: `GET users/me`, `PATCH users/me`, `GET users/{id}`, `PATCH users/{id}`, `GET users/by-email/{email}`
   - Passwords: `PATCH users/me/credentials/password` (old password or OTP token), `PATCH users/{id}/credentials/password` (admin)
   - Profiles: `GET users/me/profiles`, `GET users/{id}/profiles` (both accept `?with_skin`), `POST profile`, `GET/PATCH/DELETE profiles/{id}`
   - Admin: `POST user` (generate user + random password), `POST register/session` (create register token, max 7 days), `GET turnstile` (public instances only)
   - Registration: `POST register` (public instances + Turnstile, or any instance with a register token)

   Uses Bearer token auth, `service::Error`/`ApiResponse` wrappers, and the `Permission` bitflags system. Most user/profile endpoints accept self-or-`Management` access.

3. **Phenocryst TOTP** (`/api/`) — `POST`/`DELETE users/me/credentials/totp`, `POST verification`, `POST verification/{id}`.

### Key subsystems

- **Yggdrasil auth flow**: `authenticate` → `create_token` (optional profile selection) → `join` (cache session with serverId + IP, 30s TTL) → `hasJoined` (lookup cached session, verify token, optionally compare client IP, then return the signed profile) → `validate`/`refresh`/`invalidate`/`signout`
- **General API auth flow**: `POST /api/auth/login` (email/password or OTP token) → returns `access_token` + `client_token` + `UserPayload`. All subsequent endpoints use `Authorization: Bearer {token}`. Refresh drops the old token and issues a new one.
- **Token management**: 24h TTL, max 10 tokens per user (oldest auto-evicted), expiry checked on each use in `verify_token`.
- **Rate limiting**: token bucket per username/email (capacity 10, refill 1/sec) in Yggdrasil `authenticate`/`signout` and General API `auth/login`.
- **Session join**: cached in `KVCache` with 30s TTL. IP comparison only happens when `service.client_ip` is not `disabled`.
- **Registration access control**: private instances (`public = false`) require a `register_token`; public instances require a Turnstile token (if enabled) or accept an optional register token. Register tokens are UUIDv7s with an expiry, capped at 7 days, and expired ones are cleaned up every 24h by a background task.
- **Asset storage**: abstracted via `AssetStorage`. Local storage serves files from `data/assets` through `/assets/{...}`; S3 returns a 302 to a pre-signed URL (15 min TTL). Files are deduplicated by BLAKE3 hash with reference counting (`ref_count`).
- **Textures**: RSA SHA1-signs texture payloads for profile properties. Supports skin (default/slim) and cape textures. The frontend uploads multipart data directly to `/api/yggdrasil/api/user/profile/{uuid}/{texture_type}` (bypassing `lib/api.ts`).
- **TOTP (Phenocryst)**: `POST /api/users/me/credentials/totp` generates and **immediately stores** a secret, returning `{secret, otpauth_url}`; `DELETE` removes it. Verification: `POST /api/verification` with `{method: "totp", email}` creates a session (10 min), `POST /api/verification/{id}` with `{code}` validates the current code and returns a single-use `otp_token` (10 min) used for passwordless login and password changes.
- **Password changes**: support both old-password verification and OTP-token-based verification for resets.
- **Input validation** (`service/validation.rs`): nickname ≤ 20 chars; profile name 3–16 ASCII alphanumerics/underscore; password 8–128 bytes.
- **JSON extraction** (`service/json.rs`): wraps ntex's JSON extractor and maps errors to 400 / 413 / 415 / 422 instead of ntex defaults.
- **Base path**: `service.path` (e.g. `/aphanite`) prefixes the app; `base_path` strips slashes, `ServiceConfig::base_url()` builds `scheme://domain/path/`, and release HTML gets `window.__APHANITE_BASE` injected so `lib/api.ts` targets the right `/api`.
- **Frontend routing/state**: `App.svelte` is the management shell; on mount it validates `AUTH` and redirects to the login entry when invalid. Routes live in `lib/home-page-router.svelte.ts` (`/`, `/profiles`, `/profile/:id`, `/user`) and drive both the router and sidebar/top-bar titles. `LoginPageRouter.svelte` handles `/` and `/register` for the login entry. `not-found.html` is a standalone 404 entry.
- **Frontend state**: `AUTH` (singleton `AuthState`) persists token, user, and acquisition time to `localStorage` (`aphanite_token`, `aphanite_user`, `aphanite_token_acquired_on`); tokens older than 24h are refreshed, otherwise validated. `AUTH.profiles` is an `Option<DetailProfile[]>` loaded on demand.
- **Frontend API layer**: all General API calls go through `api.ts`, which resolves the base path, attaches the Bearer token, and returns `{ success, payload, status }` / `{ success: false, reason, status }`. Non-JSON or malformed responses throw `ApiError`.

### Migration system

Custom compile-time migration system (not toasty migrations):

- `build/build.rs` triggers `build/migrations.rs`, which reads SQL files from `migrations/turso/` and `migrations/postgres/`
- Filenames follow `{number}-{slug}.sql` (e.g. `0001-init.sql`, `0004-remove-otp-active.sql`); both dialect directories must contain the same filenames or compilation fails
- `build/migrations.rs` generates a `MigrationVersion` enum with per-database SQL scripts into `OUT_DIR`
- `src/data/migrations.rs` runs pending migrations using raw connections (Turso / tokio-postgres) **before** toasty connects, wrapping each in a transaction
- PostgreSQL uses `pg_try_advisory_lock` to prevent concurrent migration execution
- A `__aphanite_migrations` meta table tracks applied migrations
- See `migrations/README.md` for detailed rules; never edit historical migration files

### ORM (toasty)

Models are scattered across three files:

- `src/types.rs` — `User`, `Token`, `Instance`, `UserInstance`, `RegisterToken`
- `src/storage.rs` — `File`
- `src/service/yggdrasil/types.rs` — `GameProfile`, `ProfileTextures`

The schema is derived from all modules via `toasty::models!(crate::*)` in `lib.rs`. toasty reconciles tables on every startup via `db.push_schema()`, but custom migrations run first to ensure the schema is in the correct state.

## Key conventions

### Backend (Rust)

- **UUIDv7** everywhere (`Uuid::now_v7()`) for primary keys, access tokens, OTP tokens, and register tokens
- **BLAKE3** hex hashes for asset deduplication (files keyed by hash with ref counting in `ref_count`)
- **Argon2** (`argon2` crate with `password-hash` feature) for password hashing/storage in PHC string format; verification runs on `spawn_blocking` to avoid blocking the async runtime
- **RSA 4096-bit** PKCS#8 PEM private key for Yggdrasil texture signing (SHA1 with PKCS1v15); tests use a 2048-bit key for speed
- **Three error types**: `YggdrasilError` (Minecraft-compatible JSON error bodies with camelCase fields) for Yggdrasil endpoints; `service::Error { status, reason }` (serializes to `{success: false, reason}` with an explicit status code) for General API endpoints; `anyhow::Error` for internal/non-HTTP operations
- **Two success response types**: Yggdrasil endpoints return plain JSON; General API endpoints wrap values in `ApiResponse<T>` → `{success: true, payload: T}`
- **Permission system**: u32 bitflags via `Permission` enum (currently only `Permission::Management = 0b1`). Use `ToPermission::contains()`, `Permission::parse_flags()`, and `Permission::construct_number()`
- **Client IP detection**: configurable via `ReverseProxyType` (`X-Forwarded-For`, `Forwarded`, Cloudflare, etc.) or `disabled`. Custom `AphaniteClientIp` extractor (in `service/yggdrasil/types.rs`) is used by Yggdrasil handlers; invalid/missing headers yield 400
- **Routing state**: handlers use `state = crate::service::HttpState` (an alias for `ntex::web::AppState<AppState>`); `AppState` implements `FromRequest`, so handlers can take it by value
- **Comment style**: commented-out code and inline Chinese comments are left as-is (the codebase author documents intent alongside the code)
- **Config discovery**: if `config.toml` is missing, Aphanite generates one with a new RSA key and warns loudly. `APHANITE_AUTO_INIT=1` makes it write the generated config first (used by the container). `AppConfig::generate()` replaces placeholders in `config.example.toml`
- **Debug mode**: `#[cfg(debug_assertions)]` sets TLS to `false`, domain to `listen:port`, disables client-IP extraction, enables `--with-test-user`, and proxies the frontend to Vite
- **`toasty` model cloning**: model operations require a mutable `db` handle — clone with `let mut db = self.db.clone()` before querying
- **tracing**: `tracing-subscriber` with env-filter (`RUST_LOG` overrides). Debug builds with `--debug` show file paths and use `.pretty()`; non-debug builds hide code paths
- **Frontend serving is compile-time dependent**: debug = Vite proxy on `:5173`; release = embedded `web/dist`, so build the frontend before a release build
- **Tests**: integration tests use `ntex::web::test::call_service` against an in-process app and an in-memory Turso database; no external services are required

### Frontend (Svelte/TypeScript)

- **Identifier naming** follows Rust conventions (see `web/README.md`):
  - `snake_case` for local variables, functions, methods
  - `SCREAMING_SNAKE_CASE` for constants and globals
  - `PascalCase` for types, interfaces, classes, enums, components, and Svelte files
  - `kebab-case` for pure TypeScript/CSS/HTML files
  - **Do NOT** rename imported foreign identifiers
- **Styling uses Tailwind v4** with the glaucous-based theme in `app.css`; `Login.svelte`, `Home.svelte`, and `User.svelte` are good style references:
  - Use theme tokens (`text-primary`, `bg-surface`, `text-muted-foreground`, `border-border`, etc.) rather than hardcoded colors
  - Use the `@/` alias (configured for both Vite and TS) for imports
  - Use `@lucide/svelte` for icons; `@feniota/tiny-skin-viewer` for the 3D skin viewer; `overlayscrollbars-svelte` for scroll areas
  - Fonts: `Inter Variable` (UI) and `Mojangles` (Minecraft-style text)
  - Dark mode toggles `.dark` on `<body>`; preference is stored in `localStorage["aphanite.dark-mode"]`
  - `button { @apply cursor-pointer }` is global; no extra cursor classes needed
  - `cn` (twMerge + clsx) is defined in `@/lib/utils.ts` and re-exported via `@/lib/index.ts` (`@/lib.js`) only for shadcn-svelte compatibility. **Always import `cn` from `@/lib/utils.ts` in app code.**
  - **Always wrap class names in `cn()`** for extensibility.
  - **Class name hard wraps**: split long class lists into multiple string arguments to `cn()`, each line ≤ ~80 characters. `pnpm long-cn` reformats them and re-runs type checks.
- **i18n**: initialize with `await init_i18n()` before mounting (all three entry files do this). Use `t("key", opts?)` in components or `$derived`; translations live in `web/src/locales/{en,zh-CN}/translation.json` and **both files must receive new keys**. The language is persisted as `localStorage["aphanite_lang"]` (defaults to `zh-CN`, falls back to `en`). Toasts take i18n keys, not literal strings. Use `Trans.svelte` (which splits translations on `|` markers) for CJK–Latin spacing.
- **API interaction**: use functions from `api.ts`. The base path comes from `window.__APHANITE_BASE` (injected by the release server), falling back to `/api`. `AUTH.token` holds the Bearer token; login calls `AUTH.set_session()`; logout calls `AUTH.logout()`.
- **Svelte 5 runes**: use `$state`, `$derived`, `$effect`, `$props`, `$bindable` (not Svelte 4 `$:` syntax). State modules use `.svelte.ts` suffixes when runes are needed.
- **Router**: `svelte-spa-router` with hash paths (`#/path`). Two router trees: login pages (`LoginPageRouter.svelte`) and the management shell (`App.svelte` → `HomePageRouter.svelte`). Add management pages to `ROUTE_DEFS` in `lib/home-page-router.svelte.ts`; mark detail-only routes `hidden: true` to keep them out of the sidebar.
- **Toast**: `import { toast } from "@/components/toast.svelte.ts"` and pass an i18n key, e.g. `toast("toast.profile_created", { name })`. Toasts auto-dismiss after 4 seconds and pause on hover.
- **Dialogs**: use the project `Dialog.svelte` with `bind:this` for imperative control (`dialog?.open()`, `dialog?.close()`); use `* as BDialog` only when raw primitives are needed.
