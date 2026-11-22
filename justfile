_frontend-install:
    pnpm install

[group("dev")]
dev-frontend: _frontend-install
    pnpm run dev

[group("dev")]
dev-backend:
    cargo run

[group("dev")]
[parallel]
dev: dev-frontend dev-backend

[group("build")]
build-frontend: _frontend-install
    pnpm build

[group("build")]
build: build-frontend
    cargo build --release

test:
    cargo test
    node --test "tests/js/*.test.ts"

[group("lint")]
lint-frontend: _frontend-install
    pnpm exec oxfmt --check
    pnpm exec oxlint
    pnpm exec eslint
    pnpm check

[group("lint")]
lint-backend:
    cargo fmt --check
    cargo check
    cargo clippy

[group("lint")]
[parallel]
lint: lint-frontend lint-backend

[group("format")]
format-frontend: _frontend-install
    pnpm exec oxfmt

[group("format")]
format-backend:
    cargo fmt

[group("format")]
[parallel]
format: format-frontend format-backend

alias fmt := format

[group("bench")]
bench:
    pnpm build
    cargo bench --bench backend
