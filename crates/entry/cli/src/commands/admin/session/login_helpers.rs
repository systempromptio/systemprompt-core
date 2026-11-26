//! Login helpers resolving identity from cloud credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use anyhow::{Context, Result};

use systemprompt_identifiers::{ContextId, SessionId, UserId};
use systemprompt_loader::workspace::{
    CliSession, SessionBinding, SessionIdentity, SessionKey, SessionStore,
};
use systemprompt_logging::CliService;
use systemprompt_traits::{AdminAccount, AdminBootstrap, AdminGrant};

use super::login::{LoginArgs, LoginOutput};
use systemprompt_extension::cli::CommandOutput;

pub(super) async fn try_use_existing_session(
    sessions_dir: &Path,
    session_key: &SessionKey,
    issuer: &str,
    args: &LoginArgs,
    admins: &dyn AdminBootstrap,
) -> Result<Option<CommandOutput>> {
    let mut store = SessionStore::load_or_create(sessions_dir)?;

    let Some(session) = store.get_valid_session(session_key, issuer) else {
        if !args.token_only {
            CliService::info("No valid session found, creating new session...");
        }
        return Ok(None);
    };

    let session_id = session.session_id.clone();
    let user_id = session.user_id.clone();
    let user_email = session.user_email.to_string();
    let session_token = session.session_token.clone();

    let exists = admins.session_exists(&session_id).await.unwrap_or(false);

    if !exists {
        if !args.token_only {
            CliService::info(
                "Cached session is stale (not found in database), creating new session...",
            );
        }
        store.remove_session(session_key);
        store.save(sessions_dir)?;
        return Ok(None);
    }

    let output = LoginOutput {
        status: "existing".to_owned(),
        user_id,
        email: user_email,
        session_id,
        expires_in_hours: 24,
    };

    if args.token_only {
        return Ok(Some(CommandOutput::text(session_token.as_str())));
    }

    CliService::success("Using existing valid session");
    Ok(Some(CommandOutput::card_value("Admin Session", &output)))
}

pub async fn fetch_admin_user(
    admins: &dyn AdminBootstrap,
    admin_name: &str,
    is_cloud_profile: bool,
    email_override: Option<&str>,
    scope: crate::cloud_access::CloudScope<'_>,
) -> Result<AdminAccount> {
    if let Some(user) = admins
        .find_by_name(admin_name)
        .await
        .context("Failed to fetch admin user")?
    {
        if !user.is_admin() {
            anyhow::bail!(
                "User '{}' exists but is not an admin. Contact your administrator.",
                admin_name
            );
        }
        return Ok(user);
    }

    if !is_cloud_profile {
        anyhow::bail!(
            "Local admin user '{}' not found. Run 'systemprompt admin bootstrap --email \
             <email>' to create it.",
            admin_name
        );
    }

    let email = match email_override {
        Some(e) => e.to_owned(),
        None => crate::session::creation::admin_bootstrap::require_cloud_email(
            scope,
            "creating the admin user of a cloud profile",
        )
        .await?
        .as_str()
        .to_owned(),
    };

    CliService::info(&format!(
        "Admin user '{}' not found, creating it for cloud profile...",
        admin_name
    ));

    let user = admins
        .create(admin_name, &email, None)
        .await
        .context("Failed to create user")?;

    let user = admins
        .grant_admin(&user.id, AdminGrant::Only)
        .await
        .context("Failed to assign admin role")?;

    CliService::success(&format!("Created admin user: {admin_name} <{email}>"));
    Ok(user)
}

pub(super) struct SessionStoreParams<'a> {
    pub sessions_dir: &'a Path,
    pub session_key: &'a SessionKey,
    pub profile_path: &'a str,
    pub session_token: systemprompt_identifiers::SessionToken,
    pub issuer: &'a str,
    pub ttl: chrono::Duration,
    pub session_id: SessionId,
    pub context_id: Option<ContextId>,
    pub user_id: UserId,
    pub user_email: &'a str,
    pub user_type: systemprompt_models::auth::UserType,
}

pub(super) fn save_session_to_store(params: SessionStoreParams<'_>) -> Result<()> {
    let SessionStoreParams {
        sessions_dir,
        session_key,
        profile_path,
        session_token,
        issuer,
        ttl,
        session_id,
        context_id,
        user_id,
        user_email,
        user_type,
    } = params;
    let mut store = SessionStore::load_or_create(sessions_dir)?;

    let profile_dir = Path::new(profile_path).parent();
    let profile_name_str = profile_dir
        .and_then(|d| d.file_name())
        .and_then(|n| n.to_str())
        .context("Invalid profile path")?;

    let profile_name = systemprompt_identifiers::ProfileName::try_new(profile_name_str)
        .map_err(|e| anyhow::anyhow!("Invalid profile name: {}", e))?;

    let email = systemprompt_identifiers::Email::try_new(user_email)
        .map_err(|e| anyhow::anyhow!("Invalid email: {}", e))?;

    let cli_session = CliSession::builder(
        SessionBinding::new(profile_name.clone(), issuer.to_owned()),
        session_token,
        session_id,
        context_id,
        SessionIdentity::new(user_id, email, user_type),
    )
    .with_ttl(ttl)
    .with_session_key(session_key)
    .with_profile_path(profile_path)
    .build();

    store.upsert_session(session_key, cli_session);
    store.set_active_with_profile(session_key, &profile_name);
    store.save(sessions_dir)?;

    tracing::debug!(sessions_dir = %sessions_dir.display(), "session saved to index.json");
    Ok(())
}
