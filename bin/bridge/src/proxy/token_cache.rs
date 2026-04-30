use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::auth::types::HelperOutput;
use crate::proxy::forward::{ForwardError, ForwardResult};
use crate::{auth, config};

const REFRESH_TIMEOUT: Duration = Duration::from_secs(10);

pub type RefreshFn = Arc<
    dyn Fn(u64) -> Pin<Box<dyn Future<Output = Option<HelperOutput>> + Send>> + Send + Sync,
>;

struct CachedEntry {
    token: HelperOutput,
    minted_at: Instant,
}

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
    pub fn default_for_runtime() -> Self {
        Self::new(Arc::new(|threshold| {
            Box::pin(async move {
                let cfg = config::load();
                auth::read_or_refresh(&cfg, threshold).await
            })
        }))
    }

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
            .map_err(|_| ForwardError::AuthTimeout)?
            .ok_or_else(|| {
                ForwardError::Auth(
                    "no JWT available — sign in via systemprompt-bridge GUI".to_string(),
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
