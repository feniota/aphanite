use crate::service::yggdrasil::api::Session;
use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::select;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

// 自动清理间隔
const CLEAN_INTERVAL: Duration = Duration::from_secs(30);
// 登录容量
const LOGIN_CAPACITY: usize = 10;
// 登录每秒补充
const LOGIN_REFILL_PER_SEC: usize = 1;
// 会话 TTL
const SESSION_TTL_SEC: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct KVCache(Arc<KVCacheInner>);

impl KVCache {
    pub fn new() -> Self {
        let inner = Arc::new(KVCacheInner::default());
        let inner_clone = inner.clone();
        tokio::spawn(inner_clone.cleanup_thread());
        Self(inner)
    }
    pub fn try_consume(&self, user: String) -> bool {
        let mut bucket = self
            .0
            .login_rate_limit
            .entry(user)
            .or_insert_with(|| TokenBucket {
                tokens: 10.0,
                last_update: Instant::now(),
            });

        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_update).as_secs_f64();
        bucket.tokens += elapsed * LOGIN_REFILL_PER_SEC as f64;
        bucket.tokens = bucket.tokens.min(LOGIN_CAPACITY as f64);
        bucket.last_update = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
    pub fn record_session(&self, session: Session) {
        let record = SessionRecord {
            access_token: session.access_token,
            ip: session.ip,
            created_at: Instant::now(),
        };

        self.0.session_status.insert(session.server_id, record);
    }
    pub fn query_session(&self, session_id: &str) -> bool {
        let entry = match self.0.session_status.get_mut(session_id) {
            Some(e) => e,
            None => return false,
        };

        if entry.created_at.elapsed() > SESSION_TTL_SEC {
            drop(entry);
            self.0.session_status.remove(session_id);
            return false;
        }

        true
    }
}

#[derive(Default)]
struct KVCacheInner {
    login_rate_limit: DashMap<String, TokenBucket>,
    session_status: DashMap<String, SessionRecord>,
    cancellation_token: CancellationToken,
}

struct TokenBucket {
    tokens: f64,
    last_update: Instant,
}

struct SessionRecord {
    access_token: Uuid,
    ip: IpAddr,
    created_at: Instant,
}

impl KVCacheInner {
    async fn cleanup_thread(self: Arc<Self>) {
        loop {
            select! {
                _ = self.cancellation_token.cancelled() => break,
                _ = tokio::time::sleep(CLEAN_INTERVAL) => {
                    self.cleanup();
                }
            }
        }
    }
    fn cleanup(&self) {
        self.login_rate_limit
            .retain(|_, x| x.last_update.elapsed() < CLEAN_INTERVAL);
        self.session_status
            .retain(|_, x| x.created_at.elapsed() < SESSION_TTL_SEC);
    }
}

impl Drop for KVCacheInner {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}
