# Aphanite — Copilot Instructions

## Build, test, and lint

```bash
cargo build                  # Build the project
cargo build --release        # Build with optimizations
cargo test                   # Run all tests (currently only kv_cache tests in src/kv_cache.rs)
cargo test -- --nocapture    # Run tests with stdout visible
cargo clippy                 # Lint the codebase
cargo clippy -- -D warnings  # Enforce clean linting
cargo doc --no-deps --open   # View local API docs
```

Run with:

```bash
cargo run                    # Run dev server (listens on 127.0.0.1:3000, debug config)
cargo run -- --config /path/to/config.toml
cargo run -- init            # Generate a default config.toml
```

Debug/dev build extras:

```bash
cargo run -- --with-test-user       # [debug only] Creates a test user on startup
cargo run -- --debug --with-test-user
```

Create an admin user:

```bash
cargo run -- create-admin --email admin@example.com --password s3cret
```

## High-level architecture

Aphanite is an open-source, self-deployable Yggdrasil (Minecraft auth server)
with an optional Phenocryst (client instance management) extension. Built with
**Axum** (web) + **toasty** (ORM, supports SQLite and PostgreSQL).

The server exposes **two API surfaces**: the standard Yggdrasil authentication
API (for authlib-injector clients) and a custom "General API" for user/profile
management with its own token-based auth.

### Module layout

```
src/
├── main.rs                      # Entrypoint: wires up DB, migrations, storage, state, starts Axum
├── cli.rs                       # Clap CLI args + tracing setup + early-exit commands (init)
├── config.rs                    # TOML config parser + RSA key loading (PKCS#8 PEM)
├── types.rs                     # toasty models: User, Token, Instance, UserInstance + Permission bitflags
├── data.rs                      # DatabaseAccessor: verify_user, verify_token, create/delete tokens, profile queries
├── data/
│   └── migrations.rs            # Custom migration runner (rusqlite / tokio-postgres), generated via build.rs
├── kv_cache.rs                  # In-memory KVCache: token-bucket rate limiter, session join cache (30s), OTP sessions/tokens
├── storage.rs                   # AssetStorage: File model, abstraction over Local/S3, ref-counted dedup by BLAKE3 hash
├── assets/
│   └── config.example.toml      # Bundled default config (embedded via include_str!)
└── service/
    ├── mod.rs                   # Root Axum router — nests Yggdrasil (/api/yggdrasil/), General API (/api/), TOTP (/api/)
    ├── api.rs                   # General API handlers: auth/login, auth/refresh, auth/validate, user CRUD, profile CRUD, password change
    ├── types.rs                 # General API shared types: UserPayload, ProfilePayload (serde)
    ├── yggdrasil/
    │   ├── mod.rs               # Yggdrasil API router definition + Extension(AphaniteClientIp)
    │   ├── api.rs               # Yggdrasil endpoint handlers: auth (authenticate/refresh/validate/invalidate/signout), session (join/hasJoined), profiles, textures
    │   └── types.rs             # Yggdrasil types: GameProfile, ProfileTextures, ExchangeableGameProfile, TexturesPayload, SkinModel, UnhyphenatedUuid, YggdrasilError, AphaniteClientIp
    └── phenocryst/
        ├── mod.rs               # Phenocryst placeholder (empty)
        └── totp.rs              # TOTP endpoints: create_totp, active_totp, delete_totp, create_verification, complete_verification
```

Build scripts:

```
build/
├── build.rs                     # Triggers migration code generation
└── migrations.rs                # Reads migrations/{sqlite,postgres}/*.sql, generates Rust enum + match arms to OUT_DIR
```

Migrations:

```
migrations/
├── README.md
├── sqlite/0001-init.sql, 0002-add-totp-fields.sql
└── postgres/0001-init.sql, 0002-add-totp-fields.sql
```

### Two API surfaces

1. **Yggdrasil API** (`/api/yggdrasil/`) — Minecraft authlib-injector server.
   Endpoints: `authserver/authenticate`, `authserver/refresh`,
   `authserver/validate`, `authserver/invalidate`, `authserver/signout`,
   `sessionserver/session/minecraft/join`,
   `sessionserver/session/minecraft/hasJoined`,
   `sessionserver/session/minecraft/profile/{uuid}`, `api/profiles/minecraft`,
   `api/user/profile/{uuid}/{texture_type}`. Uses `YggdrasilError` (returns
   Minecraft-compatible JSON error bodies). Rate-limited per-username via
   KVCache token bucket.

2. **General API** (`/api/`) — Custom Aphanite management API. Endpoints:
   `auth/login`, `auth/refresh`, `auth/validate`, `users/{id}`, `users/me`,
   `users/{id}/credentials/password`, `users/me/credentials/password`, `user`
   (POST), `profile` (POST), `profiles/{id}` (GET/DELETE/PATCH). Uses Bearer
   token auth, `service::Error`/`ApiResponse` wrapper types, and a `Permission`
   bitflags system for access control.

### Key subsystems

- **Yggdrasil auth flow**: `authenticate` → `create_token` (optional profile
  selection) → `join` (cache session with serverId + IP, 30s TTL) → `hasJoined`
  (lookup cached session, optionally verify IP match) →
  `validate`/`refresh`/`invalidate`/`signout`
- **General API auth flow**: `POST /api/auth/login` (email/password or OTP
  token) → returns `access_token` + `client_token` + `UserPayload`. All
  subsequent endpoints use `Authorization: Bearer {access_token}`. Refresh drops
  old token and issues a new one.
- **Token management**: 24h TTL, max 10 tokens per user (oldest auto-evicted),
  expiry checked on each use in `verify_token`
- **Rate limiting**: Token-bucket per-username (capacity 10, refill 1/sec) in
  both Yggdrasil `authenticate`/`signout` and General API `auth/login`
- **Session join**: Cached in KVCache with 30s TTL for `hasJoined` lookups.
  Optionally verifies client IP matches the join IP.
- **Asset storage**: Abstracted via `AssetStorage`. Local: serves files from a
  directory. S3: generates pre-signed URLs with 15min TTL. Files deduplicated by
  BLAKE3 hash with reference counting.
- **Textures**: RSA SHA1-signs texture payloads for profile properties. Supports
  skin (default/slim) and cape textures. Upload via Yggdrasil
  `api/user/profile/{uuid}/{texture_type}`.
- **TOTP (Phenocryst)**: Two-phase setup: `POST /api/user/me/credentials/totp`
  (generates secret + otpauth URL), then `PATCH /api/user/me/credentials/totp`
  with a verified OTP token to activate. Verification: `POST /api/verification`
  (creates session), `POST /api/verification/{id}` (validates code, returns OTP
  token for subsequent operations).
- **Password changes**: Support both old-password verification and
  OTP-token-based passwordless verification for password resets.

### Migration system

Custom compile-time migration system (not toasty migrations):

- `build/build.rs` triggers `build/migrations.rs`, which reads SQL files from
  `migrations/sqlite/` and `migrations/postgres/`
- Filenames follow `{number}-{slug}.sql` format (e.g., `0001-init.sql`,
  `0002-add-totp-fields.sql`)
- `build/migrations.rs` generates a `migration_scripts.rs` file into `OUT_DIR`
  containing a `MigrationVersion` enum with per-database SQL scripts
- `src/data/migrations.rs` runs pending migrations using raw connections
  (rusqlite / tokio-postgres) **before** toasty ORM connects, wrapping each in a
  transaction
- PostgreSQL uses `pg_try_advisory_lock` to prevent concurrent migration
  execution
- A `__aphanite_migrations` meta table tracks applied migrations; new `data.rs`
  modules would handle backfilling non-SQL-computable data

### ORM (toasty)

Models are scattered across three files:

- `src/types.rs` — `User`, `Token`, `Instance`, `UserInstance`
- `src/storage.rs` — `File`
- `src/service/yggdrasil/types.rs` — `GameProfile`, `ProfileTextures`

The `toasty` schema is derived from all modules via `toasty::models!(crate::*)`
in `main.rs`. toasty creates/reconciles tables on every startup via
`db.push_schema()`, but custom migrations run first to ensure the schema is in
the correct state before toasty connects.

## Key conventions

- **UUIDv7** everywhere (`Uuid::now_v7()`) for primary keys, access tokens, and
  OTP tokens
- **BLAKE3** hex hashes for asset deduplication (files keyed by hash with ref
  counting in `ref_count`)
- **Argon2** (`argon2` crate with `password-hash` feature) for password
  hashing/storage in PHC string format
- **RSA 4096-bit** PKCS#8 PEM private key for Yggdrasil texture signing (SHA1
  with PKCS1v15)
- **Two error types**: `YggdrasilError` (returns Minecraft-compatible JSON error
  bodies with camelCase fields like `ForbiddenOperationException`) for Yggdrasil
  endpoints; `service::Error` (returns `{success: false, reason: "..."}` JSON)
  for General API endpoints; `anyhow::Error` for internal/non-http operations
- **Two success response types**: Yggdrasil endpoints return plain JSON; General
  API endpoints wrap responses in `ApiResponse<T>` →
  `{success: true, payload: T}`
- **Permission system**: u32 bitflags via `Permission` enum (currently only
  `Permission::Management = 0b1`). Use `ToPermission::contains()` trait for
  checking permissions, `Permission::from_u32()`/`to_u32()` for bit operations
- **Client IP detection**: Configurable via reverse-proxy headers or disabled.
  Custom `AphaniteClientIp` extractor (in `service/yggdrasil/types.rs`)
  registered via `Extension` layer in the Yggdrasil router. Returns `0.0.0.0`
  when disabled.
- **Comment style**: Commented-out code and inline Chinese comments are left
  as-is (the codebase author documents intent alongside the code)
- **Config discovery**: Falls back to generating a default config with a new RSA
  key if `config.toml` is missing (warns loudly). Uses `AppConfig::generate()`
  which replaces placeholders in `config.example.toml`.
- **Debug mode**: `#[cfg(debug_assertions)]` adjusts TLS defaults to `false`,
  domain to `listen:port`, `client_ip` to `disabled`, and enables
  `--with-test-user` flag
- **`toasty` model cloning**: toasty model operations require a mutable `db`
  handle — always clone with `let mut db = self.db.clone()` before querying
- **tracing**: Uses `tracing-subscriber` with env-filter (`RUST_LOG` env var).
  Debug builds show file paths with `.pretty()`. Non-debug builds hide code
  paths.
