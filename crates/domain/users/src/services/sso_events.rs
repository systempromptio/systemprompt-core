//! [`PostgresAuthEventSink`]: the core audit sink every SSO driver fans out
//! to. One `sso_login_events` row per attempt; a write failure is logged and
//! never fails the sign-in it records.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_traits::sso::{AuthEventSink, SsoLoginRefused, SsoLoginSuccess};

use crate::repository::{NewSsoLoginEvent, SsoLoginEventRepository};

pub const OUTCOME_SUCCESS: &str = "success";
pub const OUTCOME_REFUSED: &str = "refused";

#[derive(Debug, Clone)]
pub struct PostgresAuthEventSink {
    repo: SsoLoginEventRepository,
}

impl PostgresAuthEventSink {
    pub fn new(db: &DbPool) -> Self {
        Self {
            repo: SsoLoginEventRepository::new(db),
        }
    }

    async fn write(&self, event: NewSsoLoginEvent<'_>) {
        if let Err(e) = self.repo.insert_sso_login_event(&event).await {
            tracing::error!(
                provider_id = event.provider_id,
                outcome = event.outcome,
                error = %e,
                "failed to record SSO login event"
            );
        }
    }
}

#[async_trait]
impl AuthEventSink for PostgresAuthEventSink {
    async fn on_login_success(&self, event: &SsoLoginSuccess<'_>) {
        let groups_count = i32::try_from(event.identity.groups.len()).unwrap_or(i32::MAX);
        self.write(NewSsoLoginEvent {
            provider_id: event.provider_id,
            issuer: event.issuer,
            outcome: OUTCOME_SUCCESS,
            reason: None,
            user_id: Some(event.user_id),
            email: Some(event.identity.email.as_str()),
            external_sub: Some(event.identity.external_sub.as_str()),
            ip: event.ip,
            groups_count: Some(groups_count),
        })
        .await;
    }

    async fn on_login_refused(&self, event: &SsoLoginRefused<'_>) {
        self.write(NewSsoLoginEvent {
            provider_id: event.provider_id,
            issuer: event.issuer,
            outcome: OUTCOME_REFUSED,
            reason: Some(event.reason),
            user_id: None,
            email: event.email,
            external_sub: event.external_sub,
            ip: event.ip,
            groups_count: None,
        })
        .await;
    }
}
