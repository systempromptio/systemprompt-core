//! [`RunCredentialService`]: issues and revokes one run's credentials.
//!
//! Issue order is PAT, gateway session bound to the PAT, MCP token, hook
//! token; a failure after the PAT revokes what was already issued, so a failed
//! issue leaves nothing live. The tokens are minted by the deployment's
//! `TokenAuthority`, whose issuer the hook token validator checks. Revoke
//! revokes the PAT, ends the session and revokes the MCP and hook tokens'
//! `jti`s; every step runs even when an earlier one failed, and the first
//! failure is returned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::{ApiKeyId, SessionId};
use systemprompt_manifest::Config;
use systemprompt_models::auth::TokenAuthority;
use systemprompt_oauth::OAuthRepository;
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_traits::{SessionProvider, UserProvider};
use systemprompt_users::{
    ApiKeyLifetime, ApiKeyService, SessionRepository, UserRepository, UserService,
};

use super::hook_token::{HookTokenRequest, mint_hook_token, revoke_hook_token};
use super::mcp_token::{McpTokenRequest, mint_mcp_token};
use super::pat::{chrono_ttl, issue_pat, revoke_pat};
use super::session::{bind_session, end_session, open_session};
use super::{IssueRunCredentials, RunCredentials};
use crate::error::AgentRunsResult;
use crate::repository::RunCredentialRepositories;

#[derive(Clone)]
pub struct RunCredentialService {
    users: Arc<UserRepository>,
    keys: ApiKeyService,
    sessions: SessionRepository,
    creator: SessionCreationService,
    user_provider: Arc<dyn UserProvider>,
    oauth: OAuthRepository,
    authority: Arc<dyn TokenAuthority>,
}

impl std::fmt::Debug for RunCredentialService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunCredentialService")
            .field("authority", &self.authority)
            .finish_non_exhaustive()
    }
}

impl RunCredentialService {
    pub fn new(
        repos: RunCredentialRepositories,
        lifetime: ApiKeyLifetime,
        authority: Arc<dyn TokenAuthority>,
        config: Arc<Config>,
    ) -> Self {
        let RunCredentialRepositories {
            users,
            sessions,
            oauth,
        } = repos;
        let session_provider: Arc<dyn SessionProvider> = Arc::new(sessions.clone());
        let user_provider: Arc<dyn UserProvider> = Arc::new(UserService::new(Arc::clone(&users)));
        Self {
            keys: ApiKeyService::new(Arc::clone(&users), lifetime),
            sessions,
            creator: SessionCreationService::new(
                session_provider,
                Arc::clone(&user_provider),
                Arc::clone(&authority),
                config,
            ),
            user_provider,
            oauth,
            users,
            authority,
        }
    }

    async fn open_bound_session(
        &self,
        req: &IssueRunCredentials,
        api_key_id: &ApiKeyId,
        ttl: chrono::Duration,
    ) -> AgentRunsResult<SessionId> {
        let session = open_session(&self.creator, &req.user_id, ttl).await?;
        if let Err(error) = bind_session(&self.users, api_key_id, &session).await {
            end_session(&self.sessions, &session).await?;
            return Err(error);
        }
        Ok(session)
    }

    pub async fn issue(&self, req: IssueRunCredentials) -> AgentRunsResult<RunCredentials> {
        let ttl = chrono_ttl(req.ttl)?;
        let (api_key_id, pat) = issue_pat(&self.keys, &req).await?;
        let session = match self.open_bound_session(&req, &api_key_id, ttl).await {
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
            mcp_token: None,
            mcp_token_jti: None,
            mcp_token_expires_at: None,
        };
        if let Some(purpose) = req.purpose.as_deref() {
            let minted = mint_mcp_token(
                self.user_provider.as_ref(),
                McpTokenRequest {
                    authority: self.authority.as_ref(),
                    user_id: &req.user_id,
                    session_id: &credentials.gateway_session_id,
                    ttl,
                    purpose,
                },
            )
            .await;
            match minted {
                Ok(minted) => {
                    credentials.mcp_token = Some(minted.token);
                    credentials.mcp_token_jti = Some(minted.jti);
                    credentials.mcp_token_expires_at = Some(minted.expires_at);
                },
                Err(error) => {
                    self.revoke(&credentials).await?;
                    return Err(error);
                },
            }
        }
        let Some(plugin_id) = req.plugin_id.as_ref() else {
            return Ok(credentials);
        };
        let minted = mint_hook_token(
            &self.users,
            HookTokenRequest {
                authority: self.authority.as_ref(),
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

    pub async fn revoke(&self, credentials: &RunCredentials) -> AgentRunsResult<()> {
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
        let mcp = match (&credentials.mcp_token_jti, credentials.mcp_token_expires_at) {
            (Some(jti), Some(exp)) => revoke_hook_token(&self.oauth, jti, user_id, exp).await,
            _ => Ok(()),
        };
        pat.and(session).and(hook).and(mcp)
    }
}
