use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cloud::{CliSession, CredentialsBootstrap, SessionKey, SessionStore};
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken, UserId};
use systemprompt_logging::CliService;
use systemprompt_models::auth::UserType;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::{Profile, SecretsBootstrap};

use super::ProfileContext;
use crate::paths::ResolvedPaths;
use crate::session::context::CliSessionContext;

pub(super) fn try_session_from_env(profile: &Profile) -> Option<CliSessionContext> {
    if std::env::var("SYSTEMPROMPT_CLI_REMOTE").is_err() {
        return None;
    }

    let session_id = std::env::var("SYSTEMPROMPT_SESSION_ID").ok()?;
    let context_id = std::env::var("SYSTEMPROMPT_CONTEXT_ID").ok()?;
    let user_id = std::env::var("SYSTEMPROMPT_USER_ID").ok()?;
    let auth_token = std::env::var("SYSTEMPROMPT_AUTH_TOKEN").ok()?;

    let profile_name = ProfileName::new("remote");
    let email = Email::new("remote@cli.local");
    let session = CliSession::builder(
        profile_name,
        SessionToken::new(auth_token),
        SessionId::new(session_id),
        ContextId::new(context_id),
    )
    .with_user(UserId::new(user_id), email)
    .with_user_type(UserType::Admin)
    .build();

    Some(CliSessionContext {
        session,
        profile: profile.clone(),
    })
}

pub(super) fn extract_profile_name(profile_path: &Path) -> Result<String> {
    let profile_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid profile path: no parent directory"))?;
    profile_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("Invalid profile directory name"))
}

pub(super) async fn create_new_session(
    profile: &Profile,
    profile_ctx: &ProfileContext<'_>,
    session_key: &SessionKey,
    config: &crate::CliConfig,
    session_email_hint: Option<&str>,
) -> Result<CliSession> {
    if session_key.is_local() {
        return crate::session::creation::create_local_session(
            profile,
            profile_ctx,
            session_key,
            config,
            session_email_hint,
        )
        .await;
    }

    CredentialsBootstrap::try_init()
        .await
        .context("Failed to initialize credentials. Run 'systemprompt cloud auth login'.")?;

    let creds = CredentialsBootstrap::require()
        .map_err(|_| {
            anyhow::anyhow!(
                "Cloud authentication required.\n\nRun 'systemprompt cloud auth login' to \
                 authenticate."
            )
        })?
        .clone();

    crate::session::creation::create_session_for_tenant(
        &creds,
        profile,
        profile_ctx,
        session_key,
        config,
        session_email_hint,
    )
    .await
}

pub(super) fn resolve_profile_path_from_session(
    session: &CliSession,
    active_profile: Option<&str>,
) -> Result<Option<PathBuf>> {
    if let Some(expected) = active_profile {
        if session.profile_name.as_str() != expected {
            anyhow::bail!(
                "No session for active profile '{}'.\n\nRun 'systemprompt admin session login' to \
                 authenticate.",
                expected
            );
        }
    }
    match &session.profile_path {
        Some(path) if path.exists() => Ok(Some(path.clone())),
        _ => Ok(None),
    }
}

pub(super) fn resolve_profile_path_without_session(
    paths: &ResolvedPaths,
    store: &SessionStore,
    active_key: &SessionKey,
    active_profile: Option<&str>,
) -> Result<PathBuf> {
    if let Some(profile_name) = active_profile {
        let profile_dir = paths.profiles_dir().join(profile_name);
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
            let profile_hint = active_profile.unwrap_or("unknown");
            anyhow::anyhow!(
                "No session for active profile '{}'.\n\nRun 'systemprompt admin session login' to \
                 authenticate, or 'systemprompt admin session switch <profile>' to change \
                 profiles.",
                profile_hint
            )
        })
}

pub(super) fn initialize_profile_bootstraps(profile_path: &Path) -> Result<()> {
    if !ProfileBootstrap::is_initialized() {
        ProfileBootstrap::init_from_path(profile_path).with_context(|| {
            format!(
                "Failed to initialize profile from {}",
                profile_path.display()
            )
        })?;
    }

    if !SecretsBootstrap::is_initialized() {
        SecretsBootstrap::try_init().with_context(|| "Failed to initialize secrets for session")?;
    }

    Ok(())
}

pub(super) async fn try_validate_context(
    session: &mut CliSession,
    profile_name: &str,
) -> Option<CliSession> {
    let secrets = SecretsBootstrap::get()
        .map_err(|e| tracing::debug!(error = %e, "Failed to get secrets for context validation"))
        .ok()?;
    let db = Database::new_postgres(&secrets.database_url)
        .await
        .map_err(
            |e| tracing::debug!(error = %e, "Failed to connect to database for context validation"),
        )
        .ok()?;
    let db_pool = DbPool::from(Arc::new(db));
    let context_repo = ContextRepository::new(&db_pool).ok()?;

    let is_valid = context_repo
        .validate_context_ownership(&session.context_id, &session.user_id)
        .await
        .is_ok();

    if is_valid {
        return None;
    }

    CliService::warning("Session context is stale, creating new context...");

    let new_context_id = context_repo
        .create_context(
            &session.user_id,
            Some(&session.session_id),
            &format!("CLI Session - {}", profile_name),
        )
        .await
        .ok()?;

    session.set_context_id(new_context_id);
    Some(session.clone())
}
