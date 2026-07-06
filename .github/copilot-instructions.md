# Aphanite — Copilot Instructions

## Build, test, and lint

```bash
cargo build                  # Build the project
cargo build --release        # Build with optimizations
cargo test                   # Run all tests (currently only kv_cache tests)
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

## High-level architecture

Aphanite is an open-source, self-deployable Yggdrasil (Minecraft auth server) with an optional Phenocryst (client instance management) extension. Built with **Axum** (web) + **toasty** (ORM/SQLite).

### Module layout

```
src/
├── main.rs          # App entrypoint: wires up DB, storage, state, starts Axum
├── cli.rs           # Clap CLI args: --verbose, --debug, --listen, --port, --config, init subcommand
├── config.rs        # TOML config parser + RSA key loading (PKCS#8 PEM)
├── types.rs         # toasty models: User, Token, Instance, UserInstance + web Error/Result types
├── data.rs          # DatabaseAccessor: user auth, token CRUD, profile queries (24h TTL, 10 tokens/user max)
├── kv_cache.rs      # In-memory KVCache: login rate-limiting (token bucket) + session join cache (30s TTL)
├── storage.rs       # AssetStorage abstraction over Local filesystem and S3-compatible object storage
├── assets/
│   └── config.example.toml  # Bundled default config (embedded via include_str!)
└── service/
    ├── mod.rs       # Root Axum router (/api/yggdrasil, /assets)
    ├── yggdrasil/
    │   ├── mod.rs   # Yggdrasil API route definitions
    │   ├── api.rs   # Endpoint handlers (auth, session, profile, textures)
    │   └── types.rs # Yggdrasil-specific types: GameProfile, ProfileTextures, TexturesPayload, SkinModel
    └── phenocryst/
        └── mod.rs   # Placeholder for Phenocryst (not yet implemented)
```

### Key subsystems

- **Auth flow**: `authenticate` → `create_token` → `join`/`hasJoined` → `validate`/`refresh`/`invalidate`/`signout`
- **Token management**: 24h TTL, max 10 tokens per user (oldest auto-evicted), expiry checked on each use
- **Rate limiting**: Token-bucket per-username (capacity 10, refill 1/sec) in `authenticate` endpoint
- **Session join**: Cached in KVCache with 30s TTL for `hasJoined` lookups
- **Asset storage**: Abstracted via `AssetStorage`. Local: serves files from a directory. S3: generates pre-signed URLs with 15min TTL
- **Textures**: RSA SHA1-signs texture payloads for profile properties. Supports skin (default/slim) and cape textures

### ORM (toasty)

Models are scattered across three files:
- `src/types.rs` — `User`, `Token`, `Instance`, `UserInstance`
- `src/storage.rs` — `File`
- `src/service/yggdrasil/types.rs` — `GameProfile`, `ProfileTextures`

The `toasty` schema is derived from all modules via `toasty::models!(crate::*)` in `main.rs`. `db.push_schema()` creates/reconciles tables on every startup.

## Key conventions

- **UUIDv7** everywhere (`Uuid::now_v7()`) for primary keys and tokens
- **BLAKE3** hex hashes for asset deduplication (files keyed by hash with ref counting)
- **Argon2** (`argon2` crate with password-hash feature) for password hashing/storage in PHC string format
- **RSA 4096-bit** PKCS#8 PEM private key for Yggdrasil texture signing (SHA1 with PKCS1v15)
- **Error hierarchy**: `crate::Error` (axum `IntoResponse`) for generic web errors, `YggdrasilError` for auth API (returns Minecraft-compatible JSON error responses), `anyhow::Error` for internal/non-http operations
- **Client IP detection**: Configurable via reverse-proxy headers (`X-Forwarded-For`, `CF-Connecting-IP`, etc.) or disabled. Custom `AphaniteClientIp` extractor in `types.rs`
- **Comment style**: Commented-out code and inline Chinese comments are left as-is (the codebase author documents intent alongside the code)
- **Config discovery**: Falls back to generating a default config with a new RSA key if `config.toml` is missing (warns loudly)
- **Debug mode**: `#[cfg(debug_assertions)]` adjusts TLS defaults, domain, and client_ip settings automatically
