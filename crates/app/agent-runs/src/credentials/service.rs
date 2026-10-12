//! [`RunCredentialService`]: issues and revokes one run's credentials.
//!
//! Issue order is PAT, gateway session, hook token; a failure after the PAT
//! revokes what was already issued, so a failed issue leaves nothing live.
//! `issuer` is the instance's JWT issuer (`Config::jwt_issuer`), which the
//! hook token validator checks. Revoke revokes the PAT, ends the session and
//! revokes the hook token's `jti`; every step runs even when an earlier one
//! failed, and the first failure is returned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_oauth::OAuthRepository;
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_traits::{SessionProvider, UserProvider};
use systemprompt_users::{ApiKeyService, SessionRepository, UserRepository, UserService};

use super::hook_token::{HookTokenRequest, mint_hook_token, revoke_hook_token};
use super::pat::{chrono_ttl, issue_pat, revoke_pat};
use super::session::{end_session, open_session};
use super::{IssueRunCredentials, RunCredentials};
use crate::error::HeadlessResult;

#[derive(Clone)]
pub struct RunCredentialService {
    users: Arc<UserRepository>,
    keys: ApiKeyService,
    sessions: SessionRepository,
    creator: SessionCreationService,
    oauth: OAuthRepository,
    issuer: String,
}

impl std::fmt::Debug for RunCredentialService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunCredentialService")
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

impl RunCredentialService {
    pub fn new(pool: &DbPool, issuer: impl Into<String>) -> Self {
        let users = Arc::new(UserRepository::new(pool));
        let session_provider: Arc<dyn SessionProvider> = Arc::new(SessionRepository::new(pool));
        let user_provider: Arc<dyn UserProvider> = Arc::new(UserService::new(Arc::clone(&users)));
        Self {
            keys: ApiKeyService::new(Arc::clone(&users)),
            sessions: SessionRepository::new(pool),
            creator: SessionCreationService::new(session_provider, user_provider),
            oauth: OAuthRepository::new(pool),
            users,
            issuer: issuer.into(),
        }
    }

    pub async fn issue(&self, req: IssueRunCredentials) -> HeadlessResult<RunCredentials> {
        let ttl = chrono_ttl(req.ttl)?;
        let (api_key_id, pat) = issue_pat(&self.keys, &req).await?;
        let session = match open_session(&self.creator, &req.user_id, ttl).await {
            Ok(session) => session,
            Err(error) => {
                revoke_pat(&self.keys, &api_key_id, &req.user_id).await?;
                return Err(error);
            },
        };
        let mut credentials = RunCredentials {
            user_id: req.user_id.clone(),
            api_key_id,
            pat,
            gateway_session_id: session,
            hook_token: None,
            hook_token_jti: None,
            hook_token_expires_at: None,
        };
        let Some(plugin_id) = req.plugin_id.as_ref() else {
            return Ok(credentials);
        };
        let minted = mint_hook_token(
            &self.users,
            HookTokenRequest {
                issuer: &self.issuer,
                user_id: &req.user_id,
                plugin_id,
                session_id: &credentials.gateway_session_id,
                ttl,
            },
        )
        .await;
        match minted {
            Ok(minted) => {
                credentials.hook_token = Some(minted.issued.token);
                credentials.hook_token_jti = Some(minted.issued.jti);
                credentials.hook_token_expires_at = Some(minted.expires_at);
                Ok(credentials)
            },
            Err(error) => {
                self.revoke(&credentials).await?;
                Err(error)
            },
        }
    }

    pub async fn revoke(&self, credentials: &RunCredentials) -> HeadlessResult<()> {
        let user_id = &credentials.user_id;
        let pat = revoke_pat(&self.keys, &credentials.api_key_id, user_id)
            .await
            .map(|_revoked| ());
        let session = end_session(&self.sessions, &credentials.gateway_session_id).await;
        let hook = match (
            &credentials.hook_token_jti,
            credentials.hook_token_expires_at,
        ) {
            (Some(jti), Some(exp)) => revoke_hook_token(&self.oauth, jti, user_id, exp).await,
            _ => Ok(()),
        };
        pat.and(session).and(hook)
    }
}
