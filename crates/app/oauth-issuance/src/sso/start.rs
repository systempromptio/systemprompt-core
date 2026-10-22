//! An attempt is remembered server-side and bound to this browser before the
//! identity provider's redirect is handed back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Duration, Utc};
use systemprompt_identifiers::SsoRequestId;
use systemprompt_oauth::repository::SsoStateParams;
use systemprompt_oauth::services::generate_secure_token;
use systemprompt_traits::sso::{SsoError, SsoStart};

use super::cookie::CookieSpec;
use super::driver::{SsoDriver, StartOutcome};

pub const SSO_STATE_TTL_SECS: i64 = 600;

#[must_use]
pub fn sanitize_return_to(return_to: &str) -> Option<&str> {
    let candidate = return_to.trim();
    let same_origin = candidate.starts_with('/')
        && !candidate.starts_with("//")
        && !candidate.contains('\\')
        && !candidate.chars().any(char::is_control);
    same_origin.then_some(candidate)
}

impl SsoDriver {
    pub async fn start(&self, id: &str, return_to: Option<&str>) -> Result<StartOutcome, SsoError> {
        let provider = self
            .provider(id)
            .ok_or_else(|| SsoError::Unavailable(format!("no SSO provider '{id}'")))?;
        let return_to = return_to
            .and_then(sanitize_return_to)
            .unwrap_or_else(|| provider.default_return_to())
            .to_owned();
        let base = self.inner.config.api_external_url.trim_end_matches('/');
        let paths = provider.paths();
        let callback_url = format!("{base}{}", paths.callback);

        let redirect = provider
            .start(SsoStart {
                return_to: &return_to,
                callback_url: &callback_url,
            })
            .await?;
        if redirect.seed.state.is_empty() {
            return Err(SsoError::internal(format!(
                "SSO provider '{id}' returned an empty state"
            )));
        }

        let binder = generate_secure_token("sso");
        let seed = &redirect.seed;
        self.inner
            .repo
            .store_sso_state(&SsoStateParams {
                state: &seed.state,
                provider_id: id,
                return_to: &return_to,
                binder: &binder,
                request_id: seed.request_id.as_ref().map(SsoRequestId::as_str),
                nonce: seed.nonce.as_deref(),
                pkce_verifier: seed.pkce_verifier.as_deref(),
                expires_at: Utc::now() + Duration::seconds(SSO_STATE_TTL_SECS),
            })
            .await
            .map_err(SsoError::internal)?;

        Ok(StartOutcome {
            redirect_url: redirect.url,
            binder_cookie: CookieSpec::binder(
                binder,
                paths.callback,
                SSO_STATE_TTL_SECS,
                provider.return_cookie_same_site(),
                &self.inner.config.api_external_url,
            ),
        })
    }
}
