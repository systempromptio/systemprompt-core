use anyhow::Result;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::instrument;

use super::WebAuthnService;
use crate::repository::OAuthRepository;
use systemprompt_traits::UserProvider;

static WEBAUTHN_SERVICE: OnceCell<RwLock<Option<Arc<WebAuthnService>>>> = OnceCell::new();

#[derive(Debug, Copy, Clone)]
pub struct WebAuthnManager;

impl WebAuthnManager {
    #[instrument(skip(oauth_repo, user_provider))]
    pub async fn get_or_create_service(
        oauth_repo: OAuthRepository,
        user_provider: Arc<dyn UserProvider>,
    ) -> Result<Arc<WebAuthnService>> {
        let service_holder = WEBAUTHN_SERVICE.get_or_init(|| RwLock::new(None));

        let read_guard = service_holder.read().await;
        if let Some(service) = read_guard.as_ref() {
            return Ok(Arc::clone(service));
        }
        drop(read_guard);

        let mut write_guard = service_holder.write().await;
        if let Some(service) = write_guard.as_ref() {
            return Ok(Arc::clone(service));
        }

        let service = Arc::new(WebAuthnService::new(oauth_repo, user_provider)?);
        *write_guard = Some(Arc::clone(&service));
        drop(write_guard);

        Self::start_cleanup_task(Arc::clone(&service));

        Ok(service)
    }

    fn start_cleanup_task(service: Arc<WebAuthnService>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));
            loop {
                interval.tick().await;
                if let Err(e) = service.cleanup_expired_states().await {
                    tracing::error!(error = %e, "WebAuthn state cleanup error");
                }
            }
        });
    }
}
