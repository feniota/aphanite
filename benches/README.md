# HTTP endpoint benchmarks

```sh
just bench
# Equivalent:
pnpm build
cargo bench --bench backend
```

Every timed operation sends a real HTTP/1.1 request over loopback TCP to the
production ntex routes and reads the complete response body. There are no
function microbenchmarks or in-process `call_service` measurements.
The harness starts its own single-worker ntex server with the compio runtime,
compatibility middleware, mimalloc, an in-memory SQLite database and temporary
local storage. No separately running server or external credentials are needed.
Request logging and frontend fallback are omitted.

Criterion reports elapsed time and requests/second and writes HTML reports to
`target/criterion/report/index.html`. Run subsets and compare baselines on the
same machine:

```sh
cargo bench --bench backend -- http/auth --save-baseline before
cargo bench --bench backend -- http/auth --baseline before
cargo bench --bench backend -- --test
# Faster development smoke check, without release LTO or frontend embedding:
cargo bench --profile dev --bench backend -- --test
```

`--test` exercises every selected case once, checking status codes and important
response contents. It does not collect performance statistics. Use the optimized
build for comparisons; development build timings are not representative.

## Coverage

| Area                    | HTTP workloads                                                                                                |
| ----------------------- | ------------------------------------------------------------------------------------------------------------- |
| ntex baseline           | Yggdrasil metadata: routing, headers, serialization, socket I/O                                               |
| Password authentication | General API and Yggdrasil, correct and incorrect passwords; includes Argon2 and database/token work           |
| Login rate limit        | Rejection from an exhausted bucket                                                                            |
| Tokens                  | Bearer validation, missing/unknown tokens, refresh                                                            |
| User/profile database   | Current user, profile list, profile creation (untimed HTTP deletion after each iteration)                     |
| Minecraft sessions      | Join, signed hasJoined hit, miss, with 10,000 resident entries reseeded before each sample                    |
| RSA signatures          | Signed and unsigned profile responses                                                                         |
| Phenocryst              | Verification session creation, valid/invalid TOTP completion, missing session                                 |
| OTP login               | Successful consume, missing token, email mismatch, previously consumed token                                  |
| Validation              | Profile name length boundaries and invalid characters; nickname boundaries and Unicode; password boundaries   |
| Local storage           | Multipart PNG upload, new BLAKE3 object, dedup upload, deletion, 128 KiB download, 64 KiB Range, missing file |
| S3                      | Asset endpoint returns a signed 15-minute redirect; signature and expiry query parameters checked             |
| Request errors          | Malformed JSON                                                                                                |
| Concurrent traffic      | Metadata, authenticated database read, signed profile at 8 and 32 concurrent requests                         |

## Measurement boundaries

Server/runtime startup, schema creation, RSA key generation, fixture preparation,
connection warm-up and cleanup are excluded. Only request construction/send,
server processing, full response reception and response assertions are timed.
Fixtures may call internal APIs outside the timer; every measured operation is
an HTTP endpoint request. Requests reuse keep-alive connections within a sample.

Login fixtures change a separate user's email before each request, giving it a
fresh rate-limit bucket; successful logins therefore cannot silently turn into
429s. The rejection case drains refill tokens immediately before each request.
Refresh uses a disposable token, keeping the main fixture token valid. New
uploads remove the previous texture before timing; dedup uploads retain the
existing object. Password validation success includes Argon2 hashing and a real
update; it is deliberately not an isolated validator measurement.

Each concurrent iteration is a burst of 8 or 32 completed requests. Criterion's
time is **per burst**, while throughput is **requests/second**. This exercises
contention inside one server worker, including the database and signing paths.
It does not report per-request p95/p99 latency or multi-worker scaling.

The fixtures reuse integration-test setup, including a 2048-bit RSA key (normal
configuration generation uses 4096 bits). The database is in memory and file
reads usually hit the OS cache. S3 redirects are not followed; this measures
signing and redirect handling, not remote S3 upload/download/delete or network
latency. TTL expiry, TLS, proxies, external PostgreSQL, Turnstile and production
network conditions are not simulated. This is subsystem coverage through HTTP,
not an exhaustive benchmark of every route and error branch.

Session join uses a fresh server ID on every iteration. The hasJoined hit case
first joins through HTTP outside the timer, then measures only hasJoined. This
avoids reusing an expired session: inserting an existing cache key does not
refresh its TTL.
