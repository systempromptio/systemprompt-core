use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cloud::{CliSession, CredentialsBootstrap, SessionKey, SessionStore};
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken, UserId};
use systemprompt_loader::ProfileLoader;
use systemprompt_logging::CliService;
use systemprompt_models::auth::UserType;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::{Profile, SecretsBootstrap};

use super::context::CliSessionContext;
use crate::paths::ResolvedPaths;
use crate::CliConfig;

pub(super) struct ProfileContext<'a> {
    pub name: &'a str,
    pub path: PathBuf,
}

fn try_session_from_env(profile: &Profile) -> Option<CliSessionContext> {
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

async fn get_session_for_profile(
    profile_input: &str,
    config: &CliConfig,
) -> Result<CliSessionContext> {
    let (profile_path, profile) = crate::shared::resolve_profile_with_data(profile_input)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if !ProfileBootstrap::is_initialized() {
        ProfileBootstrap::init_from_path(&profile_path)
            .with_context(|| format!("Failed to initialize profile '{}'", profile_input))?;
    }

    if !SecretsBootstrap::is_initialized() {
        SecretsBootstrap::try_init().with_context(|| {
            "Failed to initialize secrets. Check your profile's secrets configuration."
        })?;
    }

    get_session_for_loaded_profile(&profile, &profile_path, config).await
}

async fn get_session_for_loaded_profile(
    profile: &Profile,
    profile_path: &Path,
    config: &CliConfig,
) -> Result<CliSessionContext> {
    if let Some(ctx) = try_session_from_env(profile) {
        return Ok(ctx);
    }

    let profile_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid profile path: no parent directory"))?;
    let profile_name = profile_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid profile directory name"))?
        .to_string();

    let tenant_id = profile.cloud.as_ref().and_then(|c| c.tenant_id.as_deref());
    let session_key = SessionKey::from_tenant_id(tenant_id);

    let sessions_dir = ResolvedPaths::discover().sessions_dir()?;

    let mut store = SessionStore::load_or_create(&sessions_dir)?;

    if let Some(mut session) = store.get_valid_session(&session_key).cloned() {
        session.touch();

        if let Some(refreshed) = try_validate_context(&mut session, &profile_name).await {
            session = refreshed;
        }

        store.upsert_session(&session_key, session.clone());
        store.save(&sessions_dir)?;
        return Ok(CliSessionContext {
            session,
            profile: profile.clone(),
        });
    }

    let profile_ctx = ProfileContext {
        name: &profile_name,
        path: profile_path.to_path_buf(),
    };

    let session = if session_key.is_local() {
        super::creation::create_local_session(profile, &profile_ctx, &session_key, config).await?
    } else {
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

        super::creation::create_session_for_tenant(
            &creds,
            profile,
            &profile_ctx,
            &session_key,
            config,
        )
        .await?
    };

    store.upsert_session(&session_key, session.clone());
    store.set_active(&session_key);
    store.save(&sessions_dir)?;

    if session.session_token.as_str().is_empty() {
        anyhow::bail!("Session token is empty. Session creation failed.");
    }

    Ok(CliSessionContext {
        session,
        profile: profile.clone(),
    })
}

async fn try_session_from_active_key(config: &CliConfig) -> Result<Option<CliSessionContext>> {
    let sessions_dir = ResolvedPaths::discover().sessions_dir()?;
    let store = SessionStore::load_or_create(&sessions_dir)?;

    let Some(ref active_key_str) = store.active_key else {
        return Ok(None);
    };

    let active_key = store.active_session_key().ok_or_else(|| {
        anyhow::anyhow!("Invalid active session key: {}", active_key_str)
    })?;

    let profile_path = if let Some(session) = store.active_session() {
        match &session.profile_path {
            Some(path) if path.exists() => path.clone(),
            _ => return Ok(None),
        }
    } else {
        let raw_session = store.get_session(&active_key);
        match raw_session.and_then(|s| s.profile_path.as_ref()).filter(|p| p.exists()) {
            Some(path) => path.clone(),
            None => {
                anyhow::bail!(
                    "Active profile has no session.\n\nRun 'systemprompt admin session login' to \
                     authenticate, or 'systemprompt admin session switch <profile>' to change \
                     profiles."
                );
            },
        }
    };

    let profile = ProfileLoader::load_from_path(&profile_path).with_context(|| {
        format!(
            "Failed to load profile from stored path: {}",
            profile_path.display()
        )
    })?;

    if !ProfileBootstrap::is_initialized() {
        ProfileBootstrap::init_from_path(&profile_path).with_context(|| {
            format!(
                "Failed to initialize profile from {}",
                profile_path.display()
            )
        })?;
    }

    if !SecretsBootstrap::is_initialized() {
        SecretsBootstrap::try_init().with_context(|| "Failed to initialize secrets for session")?;
    }

    let ctx = get_session_for_loaded_profile(&profile, &profile_path, config).await?;
    Ok(Some(ctx))
}

pub async fn get_or_create_session(config: &CliConfig) -> Result<CliSessionContext> {
    let ctx = resolve_session(config).await?;

    if config.is_interactive() {
        let tenant = ctx
            .session
            .tenant_key
            .as_ref()
            .map_or("local", systemprompt_identifiers::TenantId::as_str);
        CliService::session_context(
            ctx.session.profile_name.as_str(),
            &ctx.session.session_id,
            Some(tenant),
        );
    }

    Ok(ctx)
}

async fn resolve_session(config: &CliConfig) -> Result<CliSessionContext> {
    if let Some(ref profile_name) = config.profile_override {
        return get_session_for_profile(profile_name, config).await;
    }

    let env_profile_set = std::env::var("SYSTEMPROMPT_PROFILE").is_ok();

    if !env_profile_set {
        if let Some(ctx) = try_session_from_active_key(config).await? {
            return Ok(ctx);
        }
    }

    let profile = ProfileBootstrap::get()
        .map_err(|_| {
            anyhow::anyhow!(
                "Profile required.\n\nSet SYSTEMPROMPT_PROFILE environment variable to your \
                 profile.yaml path, or use --profile <name>."
            )
        })?
        .clone();

    let profile_path_str = ProfileBootstrap::get_path().map_err(|_| {
        anyhow::anyhow!(
            "Profile path required.\n\nSet SYSTEMPROMPT_PROFILE environment variable or use \
             --profile <name>."
        )
    })?;

    let profile_path = Path::new(profile_path_str);
    get_session_for_loaded_profile(&profile, profile_path, config).await
}

async fn try_validate_context(
    session: &mut CliSession,
    profile_name: &str,
) -> Option<CliSession> {
    let secrets = SecretsBootstrap::get().ok()?;
    let db = Database::new_postgres(&secrets.database_url).await.ok()?;
    let db_pool = DbPool::from(Arc::new(db));
    let context_repo = ContextRepository::new(db_pool);

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
