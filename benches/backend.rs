use aphanite::kv_cache::KVCache;
use aphanite::service::validation::{validate_nickname, validate_password, validate_profile_name};
use aphanite::service::yggdrasil::api::Session;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// Bound cache growth and keep samples well within the session TTL/refill interval.
const BATCH_SIZE: u64 = 256;
const EMAIL: &str = "bench@example.com";

#[derive(Clone, Copy)]
enum CacheCase {
    LoginNew,
    LoginLimited,
    SessionRecord,
    SessionHit,
    SessionMiss,
    OtpConsume,
    OtpMiss,
}

fn session(server_id: String) -> Session {
    Session {
        profile_id: Uuid::from_u128(1),
        access_token: Uuid::from_u128(2),
        server_id,
        ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
    }
}

// Each Criterion sample runs on the production ntex/compio runtime. Only the
// operation loops contribute to the returned time, excluding runtime and setup.
#[ntex::main]
async fn measure_cache(iterations: u64, case: CacheCase) -> Duration {
    let mut elapsed = Duration::ZERO;
    let mut remaining = iterations;
    while remaining > 0 {
        let count = remaining.min(BATCH_SIZE);
        let cache = KVCache::new();
        let keys: Vec<String> = (0..count).map(|i| format!("bench-{i}")).collect();
        let mut tokens = Vec::new();

        match case {
            CacheCase::LoginLimited => {
                for _ in 0..10 {
                    assert!(cache.try_consume(EMAIL).await);
                }
                assert!(!cache.try_consume(EMAIL).await);
            }
            CacheCase::SessionHit => {
                cache.record_session(session("hit".to_owned())).await;
                assert!(cache.query_session("hit").await.is_some());
            }
            CacheCase::OtpConsume => {
                for _ in 0..count {
                    tokens.push(cache.sign_otp_token(EMAIL.to_owned()).await);
                }
            }
            _ => {}
        }

        let start = Instant::now();
        match case {
            CacheCase::LoginNew => {
                for key in &keys {
                    black_box(cache.try_consume(black_box(key)).await);
                }
            }
            CacheCase::LoginLimited => {
                for _ in 0..count {
                    black_box(cache.try_consume(black_box(EMAIL)).await);
                }
            }
            CacheCase::SessionRecord => {
                for key in &keys {
                    cache.record_session(session(black_box(key).clone())).await;
                }
            }
            CacheCase::SessionHit => {
                for _ in 0..count {
                    black_box(cache.query_session(black_box("hit")).await);
                }
            }
            CacheCase::SessionMiss => {
                for key in &keys {
                    black_box(cache.query_session(black_box(key)).await);
                }
            }
            CacheCase::OtpConsume => {
                for token in &tokens {
                    black_box(cache.verify_opt_token(black_box(token), EMAIL).await);
                }
            }
            CacheCase::OtpMiss => {
                for _ in 0..count {
                    black_box(cache.verify_opt_token(black_box(&Uuid::nil()), EMAIL).await);
                }
            }
        }
        elapsed += start.elapsed();
        remaining -= count;
        drop(cache);
        // Let detached cleanup tasks exit before allocating the next batch.
        futures_lite::future::yield_now().await;
    }
    elapsed
}

fn cache_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("kv_cache");
    group.throughput(Throughput::Elements(1));
    for (name, case) in [
        ("login/new_user", CacheCase::LoginNew),
        ("login/limited", CacheCase::LoginLimited),
        ("session/record", CacheCase::SessionRecord),
        ("session/hit", CacheCase::SessionHit),
        ("session/miss", CacheCase::SessionMiss),
        ("otp/consume", CacheCase::OtpConsume),
        ("otp/miss", CacheCase::OtpMiss),
    ] {
        group.bench_function(name, |b| {
            b.iter_custom(|iterations| measure_cache(iterations, case));
        });
    }
    group.finish();
}

fn validation_benchmarks(c: &mut Criterion) {
    type Validator = fn(&str) -> Result<(), aphanite::service::Error>;
    let mut group = c.benchmark_group("validation");
    for (field, validate, inputs) in [
        (
            "profile_name",
            validate_profile_name as Validator,
            [
                ("valid", "Player_123456789"),
                ("invalid", "Player_12345678!"),
            ],
        ),
        (
            "nickname",
            validate_nickname as Validator,
            [
                ("valid", "\u{73a9}\u{5bb6}\u{6635}\u{79f0}"),
                ("invalid", "123456789012345678901"),
            ],
        ),
        (
            "password",
            validate_password as Validator,
            [("valid", "benchmark-password"), ("invalid", "short")],
        ),
    ] {
        for (case, input) in inputs {
            assert_eq!(
                validate(input).is_ok(),
                case == "valid",
                "{field}/{case}: {input:?}"
            );
            group.bench_with_input(BenchmarkId::new(field, case), &input, |b, input| {
                b.iter(|| black_box(validate(black_box(input))));
            });
        }
    }
    group.finish();
}

criterion_group!(benches, cache_benchmarks, validation_benchmarks);
criterion_main!(benches);
