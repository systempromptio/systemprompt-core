//! The service principal an in-process gateway call runs as.
//!
//! A core service calls models as its own non-human user: a service account
//! ensured through [`UserRepository::ensure_service_account`] (created active
//! with the plain `user` role, `account_kind = service`), so the gateway's
//! policy, quota windows and audit rows key on it like on any other principal.
//! Dispatch refuses a request without a session, so the principal also holds
//! a gateway session minted in-process, the same `user_sessions` row
//! `POST /api/public/gateway/sessions` mints, and re-mints it once expired.
//! The service carries no API key, so no per-key quota window applies: its
//! limits are the gateway policy's quotas on the service account.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use systemprompt_governance::policy::types::AccessScope;
use systemprompt_identifiers::{
    ClientId, ScopeDimension, ServiceName, SessionId, SessionSource, UserId,
};
use systemprompt_models::attribution::{AttributionEntry, AttributionSource, RequestAttribution};
use systemprompt_traits::{CreateSessionInput, SessionAnalytics, SessionProvider};
use systemprompt_users::{NewServiceAccount, UserRepository};
use tokio::sync::Mutex;

use super::error::PrincipalError;

pub const PURPOSE_DIMENSION: &str = "purpose";

const SESSION_TTL_HOURS: i64 = 24;
const SESSION_RENEW_MARGIN_MINUTES: i64 = 5;

/// Who a core service is when it calls the gateway.
#[derive(Debug, Clone)]
pub struct ServiceIdentity {
    pub name: ServiceName,
    pub email: String,
    pub created_by: UserId,
}

#[derive(Debug, Clone)]
struct MintedSession {
    id: SessionId,
    expires_at: DateTime<Utc>,
}

/// A resolved service account plus its in-process gateway session.
pub struct ServicePrincipal {
    user_id: UserId,
    access_scope: AccessScope,
    client_id: ClientId,
    purpose_dimension: ScopeDimension,
    sessions: Arc<dyn SessionProvider>,
    session: Mutex<Option<MintedSession>>,
}

impl std::fmt::Debug for ServicePrincipal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServicePrincipal")
            .field("user_id", &self.user_id)
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}

impl ServicePrincipal {
    pub async fn resolve(
        users: &UserRepository,
        sessions: Arc<dyn SessionProvider>,
        identity: ServiceIdentity,
    ) -> Result<Self, PrincipalError> {
        let ensured = users
            .ensure_service_account(NewServiceAccount {
                name: identity.name.as_str().to_owned(),
                email: identity.email,
                created_by: identity.created_by,
            })
            .await
            .map_err(PrincipalError::Account)?;
        if !ensured.user.is_service_account() {
            return Err(PrincipalError::NotService);
        }
        let purpose_dimension =
            ScopeDimension::try_new(PURPOSE_DIMENSION).map_err(PrincipalError::Identity)?;
        Ok(Self {
            access_scope: AccessScope::from_roles(&ensured.user.roles),
            user_id: ensured.user.id,
            client_id: ClientId::system(&identity.name),
            purpose_dimension,
            sessions,
            session: Mutex::new(None),
        })
    }

    pub const fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub const fn access_scope(&self) -> AccessScope {
        self.access_scope
    }

    pub const fn client_id(&self) -> &ClientId {
        &self.client_id
    }

    pub fn attribution(&self, purpose: &'static str) -> RequestAttribution {
        RequestAttribution {
            entries: vec![AttributionEntry {
                dimension: self.purpose_dimension.clone(),
                value: purpose.to_owned(),
                source: AttributionSource::Default,
            }],
            api_key_id: None,
        }
    }

    pub async fn session(&self) -> Result<SessionId, PrincipalError> {
        let mut slot = self.session.lock().await;
        let renew_at = Utc::now() + Duration::minutes(SESSION_RENEW_MARGIN_MINUTES);
        if let Some(current) = slot.as_ref().filter(|s| s.expires_at > renew_at) {
            return Ok(current.id.clone());
        }
        let minted = self.mint().await?;
        let id = minted.id.clone();
        *slot = Some(minted);
        drop(slot);
        Ok(id)
    }

    async fn mint(&self) -> Result<MintedSession, PrincipalError> {
        let id = SessionId::new(format!("sess_{}", uuid::Uuid::new_v4()));
        let expires_at = Utc::now() + Duration::hours(SESSION_TTL_HOURS);
        let analytics = SessionAnalytics::default();
        self.sessions
            .create_session(
                CreateSessionInput::new(&id, &analytics, SessionSource::Api, expires_at)
                    .with_user_id(&self.user_id),
            )
            .await
            .map_err(PrincipalError::Session)?;
        Ok(MintedSession { id, expires_at })
    }
}
