//! Resolution of an existing CLI session for reuse.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use systemprompt_cloud::{
    CliSession, CredentialsBootstrap, SessionBinding, SessionIdentity, SessionKey, SessionStore,
};
use systemprompt_identifiers::{Email, ProfileName};
use systemprompt_logging::CliService;
use systemprompt_manifest::Profile;
use systemprompt_models::auth::UserType;

use super::ProfileContext;
use crate::env_overrides::EnvOverrides;
use crate::session::context::CliSessionContext;
use crate::session::contexts::{SessionContextError, SessionContexts};
use crate::session::creation::SessionSource;
use systemprompt_cloud::paths::ResolvedPaths;

pub fn try_session_from_env(profile: &Profile, env: &EnvOverrides) -> Option<CliSessionContext> {
    if !env.is_remote_cli {
        return None;
    }

    let session_id = env.session.session_id.clone()?;
    let context_id = env.session.context_id.clone()?;
    let user_id = env.session.user_id.clone()?;
    let auth_token = env.session.auth_token.clone()?;

    let profile_name = ProfileName::try_new("remote").ok()?;
    let email = Email::try_new("remote@cli.local").ok()?;
    let session = CliSession::builder(
        SessionBinding::new(profile_name, profile.security.issuer.clone()),
        auth_token,
        session_id,
        context_id,
        SessionIdentity::new(user_id, email, UserType::Admin),
    )
    .build();

    Some(CliSessionContext {
        session,
        profile: profile.clone(),
    })
}

pub fn extract_profile_name(profile_path: &Path) -> Result<ProfileName> {
    let profile_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid profile path: no parent directory"))?;
    let dir_name = profile_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid profile directory name"))?;
    ProfileName::try_new(dir_name)
        .with_context(|| format!("Profile directory '{dir_name}' is not a valid profile name"))
}

pub(super) async fn create_new_session(
    profile_ctx: &ProfileContext<'_>,
    session_key: &SessionKey,
    config: &systemprompt_extension::cli::CliConfig,
    session_email_hint: Option<&str>,
    source: SessionSource<'_>,
) -> Result<CliSession> {
    let SessionSource {
        boot,
        contexts,
        admitted,
    } = source;
    let profile = boot.profile();
    if session_key.is_local() || profile.is_local_trial() {
        return crate::session::creation::create_local_session(
            profile_ctx,
            session_key,
            config,
            session_email_hint,
            source,
        )
        .await;
    }

    let creds = CredentialsBootstrap::load()
        .await
        .context("Failed to initialize credentials. Run 'systemprompt cloud auth login'.")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Cloud authentication required.\n\nRun 'systemprompt cloud auth login' to \
                 authenticate."
            )
        })?;

    crate::session::creation::create_session_for_tenant(
        crate::session::creation::TenantSessionParams {
            creds: &creds,
            profile,
            profile_ctx,
            session_key,
            config,
            session_email_hint,
            boot,
            contexts,
            admitted,
        },
    )
    .await
}

pub fn resolve_profile_path_from_session(
    session: &CliSession,
    active_profile: Option<&ProfileName>,
) -> Result<Option<PathBuf>> {
    if let Some(expected) = active_profile
        && session.profile_name != *expected
    {
        anyhow::bail!(
            "No session for active profile '{}'.\n\nRun 'systemprompt admin session login' to \
                 authenticate.",
            expected
        );
    }
    match &session.profile_path {
        Some(path) if path.exists() => Ok(Some(path.clone())),
        _ => Ok(None),
    }
}

pub fn resolve_profile_path_without_session(
    paths: &ResolvedPaths,
    store: &SessionStore,
    active_key: &SessionKey,
    active_profile: Option<&ProfileName>,
) -> Result<PathBuf> {
    if let Some(profile_name) = active_profile {
        let profile_dir = paths.profiles_dir().join(profile_name.as_str());
        let config_path = systemprompt_cloud::ProfilePath::Config.resolve(&profile_dir);
        if config_path.exists() {
            anyhow::bail!(
                "No session for active profile '{}'.\n\nRun 'systemprompt admin session login' to \
                 authenticate, or 'systemprompt admin session switch <profile>' to change \
                 profiles.",
                profile_name
            );
        }
    }

    store
        .get_session(active_key)
        .and_then(|s| s.profile_path.as_ref())
        .filter(|p| p.exists())
        .cloned()
        .ok_or_else(|| {
            let profile_hint = active_profile.map_or("unknown", ProfileName::as_str);
            anyhow::anyhow!(
                "No session for active profile '{}'.\n\nRun 'systemprompt admin session login' to \
                 authenticate, or 'systemprompt admin session switch <profile>' to change \
                 profiles.",
                profile_hint
            )
        })
}

/// What revalidating a stored session's context found.
#[derive(Debug)]
pub enum ContextCheck {
    Owned,
    Recovered(CliSession),
    Unverified(SessionContextError),
}

impl ContextCheck {
    #[must_use]
    pub const fn is_verified(&self) -> bool {
        !matches!(self, Self::Unverified(_))
    }
}

pub async fn try_validate_context(
    session: &mut CliSession,
    profile_name: &ProfileName,
    contexts: Result<SessionContexts, SessionContextError>,
) -> ContextCheck {
    match contexts {
        Ok(contexts) => revalidate_context(&contexts, session, profile_name).await,
        Err(error) => unverified(error),
    }
}

pub async fn revalidate_context(
    contexts: &SessionContexts,
    session: &mut CliSession,
    profile_name: &ProfileName,
) -> ContextCheck {
    match contexts.owns(&session.user_id, &session.context_id).await {
        Ok(true) => return ContextCheck::Owned,
        Ok(false) => CliService::warning("Session context is stale, recovering session context..."),
        Err(error) => {
            tracing::warn!(%error, "Session context ownership could not be read; treating it as invalid");
            CliService::warning(
                "Session context ownership could not be read, recovering session context...",
            );
        },
    }

    match contexts
        .issue(&session.user_id, &session.session_id, profile_name)
        .await
    {
        Ok(new_context_id) => {
            session.set_context_id(new_context_id);
            ContextCheck::Recovered(session.clone())
        },
        Err(error) => unverified(error),
    }
}

fn unverified(error: SessionContextError) -> ContextCheck {
    tracing::warn!(%error, "Session context cannot be verified");
    CliService::warning(&format!(
        "Session context cannot be verified ({error}); commands that need a conversation context \
         may fail until 'systemprompt admin session login' is run."
    ));
    ContextCheck::Unverified(error)
}
