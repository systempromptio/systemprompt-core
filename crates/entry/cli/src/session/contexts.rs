//! The CLI session's conversation context, read through the seams the
//! context-owning member provides.
//!
//! The kernel never names the agent domain: it resolves
//! `DynContextDirectory` and `DynSessionContextIssuer` from the command
//! kernel of the run's own profile, after that kernel resolves. With no admitted provider the session
//! cannot be given a context and creation fails with
//! [`SessionContextError::Unprovided`].
//!
//! `use_session_context` is the one path that rewrites a stored session's
//! context on behalf of a command: it switches the caller's session only after
//! the context directory confirms the session's user owns the context. A
//! member command reaches it only through `CommandHost::use_session_context`
//! and the `session-context` grant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_extension::Resolved;
use systemprompt_extension::cli::CommandError;
use systemprompt_identifiers::{ContextId, ProfileName, SessionId, UserId};
use systemprompt_traits::{ContextProviderError, DynContextDirectory, DynSessionContextIssuer};

use crate::host::KernelHost;

#[derive(Debug, thiserror::Error)]
pub enum SessionContextError {
    #[error("no admitted member provides {0}; the CLI session cannot hold a conversation context")]
    Unprovided(&'static str),
    #[error("the command kernel could not be resolved for the session context")]
    Kernel(#[source] CommandError),
    #[error("context {0} is not owned by the session's user")]
    NotOwned(ContextId),
    #[error("the stored CLI session could not be rewritten")]
    Store(#[source] anyhow::Error),
    #[error("the session context could not be read or issued")]
    Context(#[source] ContextProviderError),
}

/// The two seams a CLI session's context goes through.
#[derive(Clone)]
pub struct SessionContexts {
    directory: DynContextDirectory,
    issuer: DynSessionContextIssuer,
}

impl std::fmt::Debug for SessionContexts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionContexts").finish_non_exhaustive()
    }
}

impl SessionContexts {
    #[must_use]
    pub fn new(directory: DynContextDirectory, issuer: DynSessionContextIssuer) -> Self {
        Self { directory, issuer }
    }

    pub fn from_resolved(resolved: &Resolved<'_>) -> Result<Self, SessionContextError> {
        let directory = resolved
            .optional::<DynContextDirectory>()
            .ok_or(SessionContextError::Unprovided("DynContextDirectory"))?;
        let issuer = resolved
            .optional::<DynSessionContextIssuer>()
            .ok_or(SessionContextError::Unprovided("DynSessionContextIssuer"))?;
        Ok(Self::new(directory, issuer))
    }

    pub async fn for_profile(
        host: &dyn KernelHost,
        profile_path: &Path,
    ) -> Result<Self, SessionContextError> {
        tracing::debug!(profile = %profile_path.display(), "resolving the session context seams");
        let resolved = host.resolved().await.map_err(SessionContextError::Kernel)?;
        Self::from_resolved(&resolved)
    }

    pub async fn issue(
        &self,
        user_id: &UserId,
        session_id: &SessionId,
        profile_name: &ProfileName,
    ) -> Result<ContextId, SessionContextError> {
        self.issuer
            .session_context(user_id, session_id, &format!("CLI Session - {profile_name}"))
            .await
            .map_err(SessionContextError::Context)
    }

    pub async fn owns(
        &self,
        user_id: &UserId,
        context_id: &ContextId,
    ) -> Result<bool, SessionContextError> {
        self.directory
            .owned_context(user_id, context_id)
            .await
            .map(|owned| owned.is_some())
            .map_err(SessionContextError::Context)
    }
}

pub async fn use_session_context(
    host: &dyn KernelHost,
    context: ContextId,
) -> Result<(), SessionContextError> {
    let session_ctx = crate::session::get_or_create_session(host)
        .await
        .map_err(SessionContextError::Store)?;
    let profile_path = host
        .profile_path()
        .map_err(SessionContextError::Kernel)?
        .to_path_buf();
    let contexts = SessionContexts::for_profile(host, &profile_path).await?;
    if !contexts.owns(&session_ctx.session.user_id, &context).await? {
        return Err(SessionContextError::NotOwned(context));
    }
    let sessions_dir = systemprompt_cloud::paths::ResolvedPaths::discover().sessions_dir();
    let mut store = systemprompt_cloud::SessionStore::load_or_create(&sessions_dir)
        .map_err(|e| SessionContextError::Store(e.into()))?;
    let session_key = systemprompt_cloud::SessionKey::from_tenant_id(
        session_ctx
            .profile
            .cloud
            .as_ref()
            .and_then(|c| c.tenant_id.as_ref()),
    );
    let mut session = session_ctx.session;
    session.set_context_id(context);
    store.upsert_session(&session_key, session);
    store
        .save(&sessions_dir)
        .map_err(|e| SessionContextError::Store(e.into()))
}
