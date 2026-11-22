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
compatibility middleware, mimalloc, an in-memory Turso database and temporary
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
## Recorded results

### 2026-11-22 — HTTP endpoint run

- Commit (HEAD at recording): `6e006c80d1fcdaf5c8ec0f38a566c023784a680f` — Improve benches (`2026-11-22T17:11:36+08:00`). Working tree was clean before this documentation update.
- Device: Lenovo 83NN; Intel Core i9-14900HX, 24 cores / 32 logical processors; 15.7 GiB RAM reported by Windows.
- OS: Windows 11 Pro for Workstations, 64-bit, version `10.0.26300`.
- Compiler: `rustc 1.100.0 (0ef1bc386 2026-11-09)`.
- Source: user-supplied output from `just bench` (optimized benchmark); these values were transcribed, not measured again for this record.
- Criterion settings: 10 samples, 1 s warm-up, 3 s target measurement time. Warnings recommended approximately 20.2–20.9 s for sequential cases and 61.2–61.5 s for concurrent cases to complete 10 samples.
- Each bracket preserves Criterion's reported `[lower bound, estimate, upper bound]`. Throughput is requests/s (`K` = 1,000); concurrent time is per burst of 8 or 32 requests.
- Outliers: LM = low mild, LS = low severe, HM = high mild, HS = high severe. Each outlier is 10% of the 10 measurements; `—` means none reported in the supplied output.

| Benchmark | Time [lower, estimate, upper] | Requests/s [lower, estimate, upper] | Outliers |
| --- | --- | --- | --- |
| `http/ntex/meta` | [104.80 µs, 119.66 µs, 138.50 µs] | [7.2202 K, 8.3570 K, 9.5420 K] | 1 LM, 1 HS |
| `http/auth/limited` | [124.40 µs, 148.14 µs, 176.12 µs] | [5.6779 K, 6.7504 K, 8.0386 K] | 1 HM |
| `http/auth/refresh` | [451.97 µs, 519.94 µs, 604.84 µs] | [1.6533 K, 1.9233 K, 2.2125 K] | 1 HM |
| `http/token/valid` | [340.57 µs, 437.62 µs, 560.75 µs] | [1.7833 K, 2.2851 K, 2.9363 K] | 1 HS |
| `http/token/missing` | [102.15 µs, 116.70 µs, 131.52 µs] | [7.6034 K, 8.5690 K, 9.7895 K] | — |
| `http/token/unknown` | [288.08 µs, 356.31 µs, 446.64 µs] | [2.2389 K, 2.8065 K, 3.4713 K] | 1 HM, 1 HS |
| `http/user/get` | [488.46 µs, 705.00 µs, 998.49 µs] | [1.0015 K, 1.4184 K, 2.0473 K] | 1 HM |
| `http/profiles/list` | [547.21 µs, 676.09 µs, 836.74 µs] | [1.1951 K, 1.4791 K, 1.8275 K] | 1 HM, 1 HS |
| `http/profiles/create` | [502.68 µs, 569.45 µs, 644.46 µs] | [1.5517 K, 1.7561 K, 1.9893 K] | — |
| `http/session/join` | [395.96 µs, 486.99 µs, 590.74 µs] | [1.6928 K, 2.0534 K, 2.5255 K] | — |
| `http/session/hit_signed` | [2.7853 ms, 3.1083 ms, 3.6519 ms] | [273.83, 321.72, 359.03] | 1 HM, 1 HS |
| `http/session/miss` | [98.050 µs, 126.10 µs, 157.92 µs] | [6.3323 K, 7.9302 K, 10.199 K] | — |
| `http/profile/unsigned` | [414.48 µs, 723.88 µs, 1.2609 ms] | [793.06, 1.3814 K, 2.4127 K] | 1 HS |
| `http/profile/signed` | [2.9589 ms, 3.0951 ms, 3.2655 ms] | [306.24, 323.09, 337.96] | 1 HS |
| `http/profile/missing` | [275.66 µs, 338.20 µs, 428.62 µs] | [2.3331 K, 2.9568 K, 3.6277 K] | 2 HS |
| `http/totp/session/create` | [316.58 µs, 376.40 µs, 445.06 µs] | [2.2469 K, 2.6567 K, 3.1588 K] | — |
| `http/totp/session/complete` | [83.700 µs, 99.660 µs, 120.85 µs] | [8.2747 K, 10.034 K, 11.947 K] | 1 HM |
| `http/totp/session/wrong_code` | [91.970 µs, 106.99 µs, 122.60 µs] | [8.1566 K, 9.3467 K, 10.873 K] | — |
| `http/totp/session/missing` | [122.86 µs, 145.62 µs, 172.05 µs] | [5.8123 K, 6.8672 K, 8.1393 K] | 1 HM |
| `http/storage/upload/new` | [3.7968 ms, 4.4001 ms, 4.9963 ms] | [200.15, 227.27, 263.38] | — |
| `http/storage/upload/dedup` | [2.5293 ms, 2.7826 ms, 3.0890 ms] | [323.73, 359.37, 395.36] | 1 HM |
| `http/storage/delete` | [909.45 µs, 1.0763 ms, 1.2738 ms] | [785.05, 929.11, 1.0996 K] | 1 HM |
| `http/storage/download` | [835.47 µs, 915.08 µs, 1.0012 ms] | [998.81, 1.0928 K, 1.1969 K] | — |
| `http/storage/range` | [807.82 µs, 873.47 µs, 949.32 µs] | [1.0534 K, 1.1449 K, 1.2379 K] | — |
| `http/storage/missing` | [273.31 µs, 353.49 µs, 448.40 µs] | [2.2302 K, 2.8289 K, 3.6588 K] | 1 HM |
| `http/json/malformed` | [115.95 µs, 129.82 µs, 144.95 µs] | [6.8989 K, 7.7030 K, 8.6244 K] | 1 HM |
| `http/auth/password/true` | [12.981 ms, 14.590 ms, 16.733 ms] | [59.762, 68.539, 77.033] | 1 HM |
| `http/auth/password/false` | [12.076 ms, 12.968 ms, 14.019 ms] | [71.334, 77.115, 82.811] | 2 HM |
| `http/auth/yggdrasil_password/true` | [13.409 ms, 18.464 ms, 25.000 ms] | [40.000, 54.158, 74.578] | 2 HS |
| `http/auth/yggdrasil_password/false` | [13.919 ms, 16.589 ms, 19.429 ms] | [51.469, 60.279, 71.843] | — |
| `http/auth/otp/success` | [497.31 µs, 546.86 µs, 596.31 µs] | [1.6770 K, 1.8286 K, 2.0108 K] | — |
| `http/auth/otp/missing` | [138.99 µs, 174.29 µs, 215.00 µs] | [4.6512 K, 5.7376 K, 7.1948 K] | — |
| `http/auth/otp/mismatch` | [175.12 µs, 226.08 µs, 283.62 µs] | [3.5258 K, 4.4232 K, 5.7104 K] | — |
| `http/auth/otp/used` | [149.60 µs, 159.17 µs, 171.61 µs] | [5.8272 K, 6.2826 K, 6.6845 K] | 1 HM |
| `http/validation/profile_name/min` | [719.63 µs, 802.24 µs, 889.62 µs] | [1.1241 K, 1.2465 K, 1.3896 K] | — |
| `http/validation/profile_name/max` | [686.24 µs, 823.75 µs, 994.27 µs] | [1.0058 K, 1.2140 K, 1.4572 K] | 1 HM, 1 HS |
| `http/validation/profile_name/short` | [464.66 µs, 582.55 µs, 723.78 µs] | [1.3816 K, 1.7166 K, 2.1521 K] | — |
| `http/validation/profile_name/long` | [431.40 µs, 478.10 µs, 522.63 µs] | [1.9134 K, 2.0916 K, 2.3180 K] | — |
| `http/validation/profile_name/character` | [426.41 µs, 535.95 µs, 666.91 µs] | [1.4995 K, 1.8658 K, 2.3452 K] | — |
| `http/validation/nickname/min` | [598.19 µs, 681.29 µs, 766.91 µs] | [1.3039 K, 1.4678 K, 1.6717 K] | — |
| `http/validation/nickname/max` | [594.76 µs, 686.78 µs, 774.57 µs] | [1.2910 K, 1.4561 K, 1.6814 K] | — |
| `http/validation/nickname/unicode` | [621.40 µs, 709.60 µs, 823.46 µs] | [1.2144 K, 1.4092 K, 1.6093 K] | 1 HS |
| `http/validation/nickname/empty` | [451.87 µs, 533.91 µs, 634.15 µs] | [1.5769 K, 1.8730 K, 2.2130 K] | 2 HM |
| `http/validation/nickname/long` | [500.58 µs, 577.00 µs, 659.23 µs] | [1.5169 K, 1.7331 K, 1.9977 K] | 2 HM |
| `http/validation/password/min` | [12.003 ms, 12.764 ms, 13.800 ms] | [72.464, 78.342, 83.313] | 1 HS |
| `http/validation/password/max` | [13.985 ms, 16.273 ms, 19.428 ms] | [51.473, 61.451, 71.507] | 1 HM |
| `http/validation/password/short` | [330.88 µs, 355.99 µs, 383.34 µs] | [2.6087 K, 2.8091 K, 3.0222 K] | — |
| `http/validation/password/long` | [400.06 µs, 458.86 µs, 522.28 µs] | [1.9147 K, 2.1793 K, 2.4996 K] | — |
| `http/concurrent/meta/8` | [2.0373 s, 2.0441 s, 2.0505 s] | [3.9015, 3.9137, 3.9268] | — |
| `http/concurrent/database/8` | [2.0435 s, 2.0477 s, 2.0518 s] | [3.8990, 3.9069, 3.9149] | — |
| `http/concurrent/signed_profile/8` | [2.0377 s, 2.0449 s, 2.0512 s] | [3.9001, 3.9122, 3.9260] | 1 LS, 1 LM |
| `http/concurrent/meta/32` | [2.0369 s, 2.0451 s, 2.0526 s] | [15.590, 15.647, 15.710] | — |
| `http/concurrent/database/32` | [2.0431 s, 2.0486 s, 2.0533 s] | [15.585, 15.620, 15.663] | — |
| `http/concurrent/signed_profile/32` | [2.0344 s, 2.0424 s, 2.0506 s] | [15.605, 15.668, 15.730] | — |
| `http/storage/s3_signed_redirect` | [313.72 µs, 341.47 µs, 368.04 µs] | [2.7171 K, 2.9285 K, 3.1876 K] | — |

The following comparisons reproduce Criterion's output against its previous
local baseline. The baseline commit, build profile and environment were not
specified in the supplied output, so these labels do not establish a regression
or improvement between identified commits.

| Benchmark | Time change [lower, estimate, upper] | Throughput change [lower, estimate, upper] | p-value | Criterion conclusion |
| --- | --- | --- | --- | --- |
| `http/ntex/meta` | [+13.663%, +33.222%, +61.760%] | [−38.180%, −24.937%, −12.021%] | 0.01 < 0.05 | Regressed |
| `http/auth/limited` | [−28.395%, −0.5572%, +43.414%] | [−30.272%, +0.5603%, +39.656%] | 0.98 > 0.05 | No change detected |
| `http/auth/refresh` | [−26.497%, −10.418%, +10.917%] | [−9.8428%, +11.630%, +36.049%] | 0.34 > 0.05 | No change detected |
| `http/token/valid` | [−41.385%, −13.500%, +27.016%] | [−21.270%, +15.607%, +70.605%] | 0.51 > 0.05 | No change detected |
| `http/token/missing` | [−30.556%, −6.7221%, +24.018%] | [−19.366%, +7.2065%, +44.002%] | 0.70 > 0.05 | No change detected |
| `http/token/unknown` | [−3.6497%, +20.184%, +49.904%] | [−33.291%, −16.794%, +3.7879%] | 0.21 > 0.05 | No change detected |
| `http/user/get` | [+7.9306%, +61.586%, +137.96%] | [−57.976%, −38.113%, −7.3479%] | 0.06 > 0.05 | No change detected |
| `http/profiles/list` | [−24.295%, +9.0485%, +57.513%] | [−36.513%, −8.2977%, +32.092%] | 0.67 > 0.05 | No change detected |
| `http/profiles/create` | [−18.564%, −0.9463%, +19.951%] | [−16.633%, +0.9553%, +22.796%] | 0.93 > 0.05 | No change detected |
| `http/session/join` | [−75.220%, −69.031%, −62.074%] | [+163.67%, +222.90%, +303.55%] | 0.00 < 0.05 | Improved |
| `http/session/hit_signed` | [−91.014%, −88.446%, −85.383%] | [+584.15%, +765.48%, +1012.8%] | 0.00 < 0.05 | Improved |
| `http/session/miss` | [−86.702%, −80.444%, −71.006%] | [+244.89%, +411.36%, +651.97%] | 0.00 < 0.05 | Improved |
