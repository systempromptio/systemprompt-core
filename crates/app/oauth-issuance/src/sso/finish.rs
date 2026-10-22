//! Correlates an `IdP` return with its start, lets the provider validate it,
//! then runs the same checks for every provider — replay, email, policy,
//! provisioning — before minting the session. Every outcome is audited.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::IpAddr;
use std::sync::Arc;

use chrono::{Duration, Utc};
use systemprompt_traits::SessionAnalytics;
use systemprompt_traits::sso::{
    CookieSameSite, SsoError, SsoLoginRefused, SsoLoginSuccess, SsoProvider, SsoReturn,
    SsoStateContext,
};

use super::cookie::CookieSpec;
use super::driver::{FinishOutcome, SsoDriver};
use super::resolve::resolve_user;
use super::session::{BrowserSessionSubject, mint_browser_session};

const UNKNOWN_PROVIDER_LOGIN_PATH: &str = "/";
const DEFAULT_REPLAY_WINDOW_SECS: i64 = 3600;

#[derive(Debug, Default)]
struct Attempt {
    email: Option<String>,
    external_sub: Option<String>,
}

#[derive(Debug)]
pub struct SsoCallback<'a> {
    pub ret: SsoReturn,
    pub binder_cookie: Option<&'a str>,
    pub analytics: &'a SessionAnalytics,
    pub ip: Option<IpAddr>,
}

struct Correlated {
    state: Option<SsoStateContext>,
    return_to: String,
}

impl SsoDriver {
    pub async fn finish(&self, id: &str, callback: SsoCallback<'_>) -> FinishOutcome {
        let ip = callback.ip;
        let Some(provider) = self.provider(id) else {
            return FinishOutcome::Refused {
                login_path: UNKNOWN_PROVIDER_LOGIN_PATH.to_owned(),
                reason: SsoError::Unavailable(String::new()).reason_code(),
                clear_binder: self.cleared_binder(None),
            };
        };
        let clear_binder = self.cleared_binder(Some(provider.as_ref()));
        let mut attempt = Attempt::default();
        match self.run_finish(&provider, callback, &mut attempt).await {
            Ok((access_token, max_age_secs, return_to)) => FinishOutcome::Success {
                access_token,
                max_age_secs,
                return_to,
                clear_binder,
            },
            Err(err) => {
                tracing::warn!(provider_id = id, error = %err, "SSO sign-in refused");
                self.emit_refused(&SsoLoginRefused {
                    provider_id: id,
                    issuer: provider.issuer(),
                    reason: err.audit_reason(),
                    email: attempt.email.as_deref(),
                    external_sub: attempt.external_sub.as_deref(),
                    ip,
                })
                .await;
                FinishOutcome::Refused {
                    login_path: provider.login_path().to_owned(),
                    reason: err.reason_code(),
                    clear_binder,
                }
            },
        }
    }

    async fn run_finish(
        &self,
        provider: &Arc<dyn SsoProvider>,
        callback: SsoCallback<'_>,
        attempt: &mut Attempt,
    ) -> Result<(String, i64, String), SsoError> {
        let SsoCallback {
            ret,
            binder_cookie,
            analytics,
            ip,
        } = callback;
        let id = provider.id();
        let correlated = self
            .correlate(provider.as_ref(), &ret, binder_cookie)
            .await?;
        let identity = provider.finish(ret, correlated.state).await?;

        if identity.subject.trim().is_empty() {
            return Err(SsoError::NoSubject);
        }
        attempt.external_sub = Some(identity.subject.clone());

        if let Some(assertion_id) = identity.assertion_id.as_deref() {
            let expires_at = identity
                .assertion_expires_at
                .unwrap_or_else(|| Utc::now() + Duration::seconds(DEFAULT_REPLAY_WINDOW_SECS));
            let first_sight = self
                .inner
                .replay
                .claim(&format!("sso:{id}"), assertion_id, expires_at)
                .await?;
            if !first_sight {
                return Err(SsoError::Replayed);
            }
        }

        let email = identity
            .email
            .as_deref()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())
            .ok_or(SsoError::NoEmail)?;
        attempt.email = Some(email.clone());

        let mut gated = provider.policy().gate(&identity, &email)?;
        let users = Arc::clone(self.inner.session_service.user_provider());
        let resolved = resolve_user(
            users.as_ref(),
            provider.as_ref(),
            self.inner.roles.as_ref(),
            &identity,
            &gated,
        )
        .await?;
        gated.roles = resolved.roles;

        let minted = mint_browser_session(
            &self.inner.session_service,
            &self.inner.config,
            &BrowserSessionSubject {
                user_id: &resolved.user_id,
                display_name: &gated.display_name,
                email: &gated.email,
                roles: &gated.roles,
            },
            analytics,
        )
        .await?;

        self.emit_success(&SsoLoginSuccess {
            provider_id: id,
            issuer: provider.issuer(),
            user_id: &resolved.user_id,
            session_id: &minted.session_id,
            identity: &gated,
            created: resolved.created,
            ip,
        })
        .await;
        Ok((
            minted.access_token,
            minted.max_age_secs,
            correlated.return_to,
        ))
    }

    async fn correlate(
        &self,
        provider: &dyn SsoProvider,
        ret: &SsoReturn,
        binder_cookie: Option<&str>,
    ) -> Result<Correlated, SsoError> {
        let state = ret
            .param("state")
            .or_else(|| ret.param("RelayState"))
            .filter(|s| !s.is_empty());
        let row = match state {
            Some(state) => self
                .inner
                .repo
                .consume_sso_state(provider.id(), state)
                .await
                .map_err(SsoError::internal)?,
            None => None,
        };

        let Some(row) = row else {
            if !provider.allow_unsolicited() {
                return Err(if state.is_some() {
                    SsoError::StateLost
                } else {
                    SsoError::UnsolicitedNotAllowed
                });
            }
            return Ok(Correlated {
                state: None,
                return_to: provider.default_return_to().to_owned(),
            });
        };

        let bound = match binder_cookie {
            Some(binder) => row.binder_matches(binder).map_err(SsoError::internal)?,
            None => provider.allow_unsolicited(),
        };
        if !bound {
            return Err(SsoError::StateLost);
        }
        Ok(Correlated {
            state: Some(SsoStateContext {
                request_id: row.request_id.map(|id| id.as_str().to_owned()),
                nonce: row.nonce,
                pkce_verifier: row.pkce_verifier,
            }),
            return_to: row.return_to,
        })
    }

    fn cleared_binder(&self, provider: Option<&dyn SsoProvider>) -> CookieSpec {
        let (path, same_site) = provider.map_or(("/", CookieSameSite::Lax), |p| {
            (p.paths().callback, p.return_cookie_same_site())
        });
        CookieSpec::binder(
            String::new(),
            path,
            0,
            same_site,
            &self.inner.config.api_external_url,
        )
    }
}
