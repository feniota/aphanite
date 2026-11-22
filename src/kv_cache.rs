use crate::service::phenocryst::totp::OtpSession;
use crate::service::yggdrasil::api::Session;
use scc::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

// 自动清理间隔
const CLEAN_INTERVAL: Duration = Duration::from_secs(30);
// 登录容量
const LOGIN_CAPACITY: usize = 10;
// 登录每秒补充
const LOGIN_REFILL_PER_SEC: usize = 1;
// 会话 TTL
const SESSION_TTL_SEC: Duration = Duration::from_secs(30);
// OTP Token TTL
const OTP_TOKEN_TTL: Duration = Duration::from_mins(10);

#[derive(Clone)]
pub struct KVCache {
    inner: Arc<KVCacheInner>,
    _cleanup: Arc<async_channel::Sender<()>>,
}

impl Default for KVCache {
    fn default() -> Self {
        Self::new()
    }
}

impl KVCache {
    pub fn new() -> Self {
        let inner = Arc::new(KVCacheInner::default());

        let (cleanup, exit) = async_channel::bounded(1);
        ntex::rt::spawn(KVCacheInner::cleanup_thread(inner.clone(), exit));
        let _cleanup = Arc::new(cleanup);
        Self { inner, _cleanup }
    }
    pub async fn try_consume(&self, user: &str) -> bool {
        let mut occupied = self
            .inner
            .login_rate_limit
            .entry_async(user.to_owned())
            .await
            .or_insert_with(|| TokenBucket {
                tokens: LOGIN_CAPACITY as f64,
                last_update: Instant::now(),
            });

        let bucket = occupied.get_mut();
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
    pub async fn record_session(&self, session: Session) {
        let record = SessionRecord {
            profile_id: session.profile_id,
            access_token: session.access_token,
            ip: session.ip,
            created_at: Instant::now(),
        };

        let _ = self
            .inner
            .session_status
            .insert_async(session.server_id, record)
            .await;
    }
    pub async fn query_session(&self, session_id: &str) -> Option<Session> {
        if self
            .inner
            .session_status
            .remove_if_async(session_id, |r| r.created_at.elapsed() > SESSION_TTL_SEC)
            .await
            .is_some()
        {
            return None;
        }

        self.inner
            .session_status
            .read_async(session_id, |_, record| Session {
                profile_id: record.profile_id,
                server_id: session_id.to_string(),
                access_token: record.access_token,
                ip: record.ip,
            })
            .await
    }
    pub async fn insert_otp_session(&self, session: OtpSession) -> Uuid {
        let id = Uuid::now_v7();
        let _ = self.inner.otp_sessions.insert_async(id, session).await;
        id
    }
    pub async fn query_otp_session(&self, id: &Uuid) -> Option<OtpSession> {
        if self
            .inner
            .otp_sessions
            .remove_if_async(id, |s| Instant::now() >= s.expired_at)
            .await
            .is_some()
        {
            return None;
        }

        self.inner
            .otp_sessions
            .read_async(id, |_, s| s.clone())
            .await
    }
    pub async fn sign_otp_token(&self, user_email: String) -> Uuid {
        let token = Uuid::now_v7();
        let _ = self
            .inner
            .otp_tokens
            .insert_async(
                token,
                OtpToken {
                    user_email,
                    created_at: Instant::now(),
                },
            )
            .await;
        token
    }
    pub async fn verify_opt_token(&self, token: &Uuid, user_email: &str) -> bool {
        self.inner
            .otp_tokens
            .remove_if_async(token, |t| {
                t.user_email == user_email && t.created_at.elapsed() < OTP_TOKEN_TTL
            })
            .await
            .is_some()
    }
}

#[derive(Default)]
struct KVCacheInner {
    login_rate_limit: HashMap<String, TokenBucket>,
    session_status: HashMap<String, SessionRecord>,
    otp_sessions: HashMap<Uuid, OtpSession>,
    otp_tokens: HashMap<Uuid, OtpToken>,
}

struct TokenBucket {
    tokens: f64,
    last_update: Instant,
}

struct SessionRecord {
    profile_id: Uuid,
    access_token: Uuid,
    ip: IpAddr,
    created_at: Instant,
}

struct OtpToken {
    user_email: String,
    created_at: Instant,
}

impl KVCacheInner {
    async fn cleanup_thread(self: Arc<Self>, exit: async_channel::Receiver<()>) {
        futures_lite::future::race(
            async {
                loop {
                    self.cleanup().await;
                    ntex::time::sleep(CLEAN_INTERVAL).await;
                }
            },
            async {
                let _ = exit.recv().await;
            },
        )
        .await;
    }
    async fn cleanup(&self) {
        self.login_rate_limit
            .retain_async(|_, x| x.last_update.elapsed() < CLEAN_INTERVAL)
            .await;
        self.session_status
            .retain_async(|_, x| x.created_at.elapsed() < SESSION_TTL_SEC)
            .await;
        self.otp_sessions
            .retain_async(|_, x| Instant::now() < x.expired_at)
            .await;
        self.otp_tokens
            .retain_async(|_, x| x.created_at.elapsed() < OTP_TOKEN_TTL)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[ntex::test]
    async fn kv_cache_burst_then_limit() {
        let cache = KVCache::new();
        let user = Uuid::new_v4();

        let mut handles = vec![];
        for _ in 0..20 {
            let cache = cache.clone();
            handles.push(ntex::rt::spawn(async move {
                cache.try_consume(&user.to_string()).await
            }));
        }

        let mut success = 0;
        for h in handles {
            if h.await.unwrap() {
                success += 1;
            }
        }

        println!("burst success = {}", success);

        assert!(success <= LOGIN_CAPACITY);

        ntex::time::sleep(Duration::from_secs(2)).await;

        let mut second = 0;
        for _ in 0..5 {
            if cache.try_consume(&user.to_string()).await {
                second += 1;
            }
        }

        println!("after refill success = {}", second);

        assert!(second > 0);
    }
}
