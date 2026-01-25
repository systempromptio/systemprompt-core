use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use systemprompt_cloud::paths::{get_cloud_paths, CloudPath};
use systemprompt_cloud::{
    CliSession, CredentialsBootstrap, ProfilePath, ProjectContext, SessionKey, SessionStore,
};
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken, UserId};
use systemprompt_loader::ProfileLoader;
use systemprompt_logging::CliService;
use systemprompt_models::auth::UserType;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::Profile;

use super::context::CliSessionContext;
use crate::CliConfig;

pub(super) struct ProfileContext<'a> {
    pub dir: &'a Path,
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

pub(super) fn resolve_session_paths(
    project_ctx: &ProjectContext,
) -> Result<(PathBuf, Option<PathBuf>)> {
    if project_ctx.systemprompt_dir().exists() {
        let sessions_dir = project_ctx.sessions_dir();
        let legacy_path = project_ctx.local_session();
        Ok((sessions_dir, Some(legacy_path)))
    } else {
        let cloud_paths = get_cloud_paths()
            .context("Failed to resolve cloud paths from profile configuration")?;
        let sessions_dir = cloud_paths.resolve(CloudPath::SessionsDir);
        let legacy_path = cloud_paths.resolve(CloudPath::CliSession);
        Ok((sessions_dir, Some(legacy_path)))
    }
}

fn resolve_profile_by_name(profile_name: &str) -> Result<(PathBuf, Profile)> {
    let project_ctx = ProjectContext::discover();
    let profile_dir = project_ctx.profile_dir(profile_name);
    let profile_config_path = ProfilePath::Config.resolve(&profile_dir);

    if !profile_config_path.exists() {
        anyhow::bail!(
            "Profile '{}' not found.\n\nAvailable profiles can be listed with: systemprompt admin \
             session list",
            profile_name
        );
    }

    let profile = ProfileLoader::load_from_path(&profile_config_path)
        .with_context(|| format!("Failed to load profile '{}'", profile_name))?;

    Ok((profile_config_path, profile))
}

async fn get_session_for_profile(
    profile_name: &str,
    config: &CliConfig,
) -> Result<CliSessionContext> {
    let (profile_path, profile) = resolve_profile_by_name(profile_name)?;

    if !ProfileBootstrap::is_initialized() {
        ProfileBootstrap::init_from_path(&profile_path)
            .with_context(|| format!("Failed to initialize profile '{}'", profile_name))?;
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

    let project_ctx = ProjectContext::discover();
    let (sessions_dir, legacy_path) = resolve_session_paths(&project_ctx)?;

    let mut store = SessionStore::load_or_create(&sessions_dir, legacy_path.as_deref())?;

    if let Some(mut session) = store.get_valid_session(&session_key).cloned() {
        session.touch();
        store.upsert_session(&session_key, session.clone());
        store.save(&sessions_dir)?;
        return Ok(CliSessionContext {
            session,
            profile: profile.clone(),
        });
    }

    let profile_ctx = ProfileContext {
        dir: profile_dir,
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
    let project_ctx = ProjectContext::discover();
    let (sessions_dir, legacy_path) = resolve_session_paths(&project_ctx)?;
    let store = SessionStore::load_or_create(&sessions_dir, legacy_path.as_deref())?;

    let Some(active_session) = store.active_session() else {
        return Ok(None);
    };

    let profile_path = match &active_session.profile_path {
        Some(path) if path.exists() => path,
        _ => return Ok(None),
    };

    let profile = ProfileLoader::load_from_path(profile_path).with_context(|| {
        format!(
            "Failed to load profile from stored path: {}",
            profile_path.display()
        )
    })?;

    if !ProfileBootstrap::is_initialized() {
        ProfileBootstrap::init_from_path(profile_path).with_context(|| {
            format!(
                "Failed to initialize profile from {}",
                profile_path.display()
            )
        })?;
    }

    let ctx = get_session_for_loaded_profile(&profile, profile_path, config).await?;
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
