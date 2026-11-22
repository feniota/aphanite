# Backend benchmarks

Run from the repository root on stable Rust:

```sh
pnpm build
cargo bench --bench backend
```

The frontend build is required because the optimized Rust build embeds `web/dist`.
No running server, database, credentials or external services are needed.
`just bench` builds the frontend and runs the benchmarks with the same commands.

Criterion prints timings, confidence intervals and throughput (cache operations
per second), and writes HTML reports to `target/criterion/report/index.html`.
Use filters and saved baselines to compare changes on the same machine:

```sh
cargo bench --bench backend -- kv_cache --save-baseline before
# Make the change being evaluated.
cargo bench --bench backend -- kv_cache --baseline before
cargo bench --bench backend -- --test
```

The final command executes each benchmark once as a smoke check, without
collecting statistics. Benchmark results are machine dependent; close other
CPU-heavy applications when comparing baselines.

## Coverage and measurement

- Login rate limiter: insert and admit a new user; reject an exhausted bucket.
- Minecraft sessions: record a new session; query a live session or a missing key.
- OTP tokens: successfully consume a pre-issued, single-use token; reject a missing token.
- Input validation: valid and invalid profile names, nicknames and passwords.

Cache samples run on ntex's configured compio runtime, with mimalloc as in the
server binary. Each sample uses fresh caches in batches of at most 256 operations
to bound memory and avoid session expiry/rate-limit refills during ordinary runs.
Runtime startup, fixture generation, cache setup, token issuance, and cache
teardown are excluded from the reported operation times. The cleanup tasks get
a chance to exit between batches. Recording a session includes allocating its
owned server ID; consuming an OTP token includes removing its entry.

These are isolated, sequential microbenchmarks. They do not measure HTTP
latency, database work, contention, password hashing or end-to-end login throughput.
