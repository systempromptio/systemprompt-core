use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use systemprompt_identifiers::SessionId;

use crate::auth::types::HelperOutput;
use crate::proxy::forward::{ForwardError, ForwardResult};
use crate::{auth, config};

const REFRESH_TIMEOUT: Duration = Duration::from_secs(10);

pub type RefreshFn =
    Arc<dyn Fn(u64) -> Pin<Box<dyn Future<Output = Option<HelperOutput>> + Send>> + Send + Sync>;

struct CachedEntry {
    token: HelperOutput,
    minted_at: Instant,
}

#[expect(
    missing_debug_implementations,
    reason = "holds a `dyn Fn -> Pin<Box<Future>>` refresh callback; cannot derive Debug"
)]
pub struct TokenCache {
    cached: Mutex<Option<CachedEntry>>,
    refresh_lock: Mutex<()>,
    refresh: RefreshFn,
}

impl TokenCache {
    #[must_use]
    pub fn new(refresh: RefreshFn) -> Self {
        Self {
            cached: Mutex::new(None),
            refresh_lock: Mutex::new(()),
            refresh,
        }
    }

    #[must_use]
    pub fn default_for_runtime(session_id: SessionId) -> Self {
        Self::new(Arc::new(move |threshold| {
            let session_id = session_id.clone();
            Box::pin(async move {
                let cfg = config::load();
                auth::read_or_refresh(&cfg, threshold, &session_id).await
            })
        }))
    }

    #[expect(
        clippy::significant_drop_tightening,
        reason = "refresh_guard intentionally held to serialise concurrent refreshes"
    )]
    pub async fn current(&self, refresh_threshold_secs: u64) -> ForwardResult<HelperOutput> {
        if let Some(token) = self.peek_fresh(refresh_threshold_secs).await {
            return Ok(token);
        }

        let _refresh_guard = self.refresh_lock.lock().await;

        if let Some(token) = self.peek_fresh(refresh_threshold_secs).await {
            return Ok(token);
        }

        let refresh = Arc::clone(&self.refresh);
        let token = tokio::time::timeout(REFRESH_TIMEOUT, refresh(refresh_threshold_secs))
            .await
            .map_err(|_elapsed| ForwardError::AuthTimeout)?
            .ok_or_else(|| {
                ForwardError::Auth(
                    "no JWT available — sign in via systemprompt-bridge GUI".to_owned(),
                )
            })?;

        tracing::info!("token cache refresh");

        let mut guard = self.cached.lock().await;
        *guard = Some(CachedEntry {
            token: token.clone(),
            minted_at: Instant::now(),
        });
        Ok(token)
    }

    pub async fn invalidate(&self) {
        let mut guard = self.cached.lock().await;
        if guard.is_some() {
            tracing::info!("token cache invalidated (upstream rejected JWT)");
            *guard = None;
        }
    }

    #[expect(
        clippy::significant_drop_tightening,
        reason = "guard scope is the whole function; entry borrows from it"
    )]
    async fn peek_fresh(&self, refresh_threshold_secs: u64) -> Option<HelperOutput> {
        let guard = self.cached.lock().await;
        let entry = guard.as_ref()?;
        let age_secs = entry.minted_at.elapsed().as_secs();
        if age_secs.saturating_add(refresh_threshold_secs) < entry.token.ttl {
            tracing::debug!(cached_age_secs = age_secs, "token cache hit");
            Some(entry.token.clone())
        } else {
            None
        }
    }
}
