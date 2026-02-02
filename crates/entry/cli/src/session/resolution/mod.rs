mod helpers;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use systemprompt_cloud::{SessionKey, SessionStore};
use systemprompt_loader::ProfileLoader;
use systemprompt_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::{Profile, SecretsBootstrap};

use super::context::CliSessionContext;
use crate::paths::ResolvedPaths;
use crate::CliConfig;
use helpers::{
    create_new_session, extract_profile_name, initialize_profile_bootstraps,
    resolve_profile_path_from_session, resolve_profile_path_without_session, try_session_from_env,
    try_validate_context,
};

pub(super) struct ProfileContext<'a> {
    pub name: &'a str,
    pub path: PathBuf,
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

    let profile_name = extract_profile_name(profile_path)?;
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

    let session_email_hint = store
        .get_session(&session_key)
        .map(|s| s.user_email.to_string());

    let profile_ctx = ProfileContext {
        name: &profile_name,
        path: profile_path.to_path_buf(),
    };

    let session = create_new_session(
        profile,
        &profile_ctx,
        &session_key,
        config,
        session_email_hint.as_deref(),
    )
    .await?;

    store.upsert_session(&session_key, session.clone());
    store.set_active_with_profile(&session_key, &profile_name);
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
    let paths = ResolvedPaths::discover();
    let sessions_dir = paths.sessions_dir()?;
    let store = SessionStore::load_or_create(&sessions_dir)?;

    let Some(ref active_key_str) = store.active_key else {
        return Ok(None);
    };

    let active_key = store
        .active_session_key()
        .ok_or_else(|| anyhow::anyhow!("Invalid active session key: {}", active_key_str))?;

    let active_profile = store.active_profile_name.as_deref();

    let profile_path = if let Some(session) = store.active_session() {
        match resolve_profile_path_from_session(session, active_profile)? {
            Some(path) => path,
            None => return Ok(None),
        }
    } else {
        resolve_profile_path_without_session(&paths, &store, &active_key, active_profile)?
    };

    let profile = ProfileLoader::load_from_path(&profile_path).with_context(|| {
        format!(
            "Failed to load profile from stored path: {}",
            profile_path.display()
        )
    })?;

    initialize_profile_bootstraps(&profile_path)?;

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
        CliService::session_context_with_url(
            ctx.session.profile_name.as_str(),
            &ctx.session.session_id,
            Some(tenant),
            Some(&ctx.profile.server.api_external_url),
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
