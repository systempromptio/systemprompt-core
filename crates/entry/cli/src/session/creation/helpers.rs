//! Helpers minting CLI session rows and their analytics context.
//!
//! A local install's admin is resolved by **name**, never by email. An email is
//! an attribute of a person, not a key: the local-trial path used to look up
//! the literal `admin@localhost.dev`, which forced a migration to write that
//! same string into `users.email` so the two would meet, and the address was
//! then displayed as the operator's identity — including on the bridge
//! device-link consent screen, immediately above a button that mints a durable
//! personal access token. `system_admin.username` is the key the runtime
//! already resolves on, so resolving by it agrees with the runtime by
//! construction and leaves `email` free to hold something true. That path
//! deliberately does not provision: on a local install a missing admin means
//! bootstrap has not run, and inventing one is what produced the fabricated
//! identity in the first place. Every address returned by
//! `resolve_credentialed_user_email` comes from a session hint or from cloud
//! credentials — real data either way.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cloud::{
    CliSession, CloudCredentials, CredentialsBootstrap, SessionBinding, SessionIdentity, SessionKey,
};
use systemprompt_config::SecretsBootstrap;
use systemprompt_database::{Database, DbPool, PoolConfig};
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken};
use systemprompt_models::auth::{Permission, RateLimitTier, UserType};
use systemprompt_security::{SessionGenerator, SessionParams};
use systemprompt_users::{UserRepository, UserService};

use crate::session::resolution::ProfileContext;

pub(super) struct ResolvedSecrets {
    pub database_url: String,
    pub database_write_url: Option<String>,
}

pub(super) fn load_secrets() -> Result<ResolvedSecrets> {
    let secrets = SecretsBootstrap::get().map_err(|e| {
        anyhow::anyhow!(
            "Secrets not initialized: {}\n\nEnsure your profile has a valid secrets \
             configuration.\nCheck that secrets.json exists or environment variables are set.",
            e
        )
    })?;

    Ok(ResolvedSecrets {
        database_url: secrets.database_url.clone(),
        database_write_url: secrets.database_write_url.clone(),
    })
}

pub(super) async fn connect_database(secrets: &ResolvedSecrets) -> Result<DbPool> {
    let db = Database::from_config_with_write(
        "postgres",
        &secrets.database_url,
        secrets.database_write_url.as_deref(),
        &PoolConfig::default(),
    )
    .await
    .context("Failed to connect to database")?;
    Ok(DbPool::from(Arc::new(db)))
}

pub async fn get_or_create_admin(
    db_pool: &DbPool,
    email: &str,
    context_type: &str,
) -> Result<systemprompt_users::User> {
    let user_service = UserService::new(Arc::new(UserRepository::new(db_pool)?));

    if let Some(user) = user_service
        .find_by_email(email)
        .await
        .context("Failed to query user by email")?
    {
        if user.is_admin() {
            return Ok(user);
        }

        tracing::info!(email = %email, context = %context_type, "Promoting existing user to admin");

        return user_service
            .assign_roles(&user.id, &["admin".to_owned()])
            .await
            .context("Failed to assign admin role to existing user");
    }

    let name = email.split('@').next().unwrap_or("admin").to_owned();

    tracing::info!(email = %email, name = %name, context = %context_type, "Auto-provisioning user");

    let user = match user_service
        .create_if_absent(&name, email, None, None)
        .await
        .with_context(|| format!("Failed to create user in {context_type} database"))?
    {
        Some(user) => user,
        None => user_service
            .find_by_email(email)
            .await
            .context("Failed to query user by email")?
            .with_context(|| format!("User {email} vanished between provisioning and lookup"))?,
    };

    user_service
        .assign_roles(&user.id, &["admin".to_owned()])
        .await
        .context("Failed to assign admin role to new user")
}

pub fn generate_admin_token(
    issuer: &str,
    user: &systemprompt_users::User,
    session_id: &SessionId,
) -> Result<SessionToken> {
    let generator = SessionGenerator::new(issuer);
    generator
        .generate(&SessionParams {
            user_id: &user.id,
            session_id,
            email: &user.email,
            duration: ChronoDuration::hours(crate::session::api::DEFAULT_CLI_SESSION_HOURS),
            user_type: UserType::Admin,
            permissions: vec![Permission::Admin],
            roles: vec!["admin".to_owned()],
            attributes: std::collections::BTreeMap::new(),
            rate_limit_tier: RateLimitTier::Admin,
        })
        .context("Failed to generate session token")
}

pub(super) async fn create_cli_context(
    db_pool: DbPool,
    user: &systemprompt_users::User,
    session_id: &SessionId,
    profile_name: &str,
) -> Result<ContextId> {
    let context_repo = ContextRepository::new(&db_pool)?;
    context_repo
        .get_or_create_cli_context(
            &user.id,
            session_id,
            &format!("CLI Session - {}", profile_name),
        )
        .await
        .context("Failed to create CLI context")
}

pub(super) struct SessionComponents {
    pub session_token: SessionToken,
    pub session_id: SessionId,
    pub context_id: ContextId,
}

pub(super) fn build_cli_session(
    profile_ctx: &ProfileContext<'_>,
    session_key: &SessionKey,
    components: SessionComponents,
    admin_user: &systemprompt_users::User,
    issuer: &str,
) -> Result<CliSession> {
    let profile_name = ProfileName::try_new(profile_ctx.name)
        .map_err(|e| anyhow::anyhow!("Invalid profile name: {}", e))?;
    let email =
        Email::try_new(&admin_user.email).map_err(|e| anyhow::anyhow!("Invalid email: {}", e))?;

    Ok(CliSession::builder(
        SessionBinding::new(profile_name, issuer.to_owned()),
        components.session_token,
        components.session_id,
        components.context_id,
        SessionIdentity::new(admin_user.id.clone(), email, UserType::Admin),
    )
    .with_session_key(session_key)
    .with_profile_path(profile_ctx.path.clone())
    .build())
}

pub async fn resolve_local_admin(
    db_pool: &DbPool,
    admin_name: &str,
) -> Result<systemprompt_users::User> {
    let user_service = UserService::new(Arc::new(UserRepository::new(db_pool)?));

    let user = user_service
        .find_by_name(admin_name)
        .await
        .context("Failed to query the local admin user by name")?
        .with_context(|| {
            format!(
                "Local admin user '{admin_name}' not found.\n\nRun 'systemprompt admin bootstrap \
                 --email <your email>' to create it with a real address."
            )
        })?;

    if !user.is_active() {
        anyhow::bail!("Local admin user '{admin_name}' exists but is not active.");
    }
    if !user.is_admin() {
        anyhow::bail!(
            "User '{admin_name}' exists but does not hold the admin role. Run 'systemprompt admin \
             bootstrap' to repair it."
        );
    }

    Ok(user)
}

pub(super) async fn resolve_credentialed_user_email(
    session_email_hint: Option<&str>,
) -> Result<Email> {
    if let Some(email) = session_email_hint {
        return Email::try_new(email).context("session email hint is not a valid email address");
    }

    CredentialsBootstrap::try_init()
        .await
        .context("Failed to initialize credentials. Run 'systemprompt cloud auth login'.")?;

    let creds = CredentialsBootstrap::require().map_err(|_e| {
        anyhow::anyhow!(
            "Cloud authentication required for new sessions.\n\nRun 'systemprompt cloud auth \
             login' to authenticate."
        )
    })?;
    Ok(creds.user_email.clone())
}

pub(super) async fn resolve_admin_with_fallback(
    db_pool: &DbPool,
    user_email: &str,
    session_email_hint: Option<&str>,
    context_type: &str,
) -> Result<systemprompt_users::User> {
    match get_or_create_admin(db_pool, user_email, context_type).await {
        Ok(user) => Ok(user),
        Err(e) if session_email_hint.is_some() => {
            tracing::warn!(
                email = %user_email,
                error = %e,
                "Session user lookup failed, falling back to cloud credentials"
            );
            if let Err(init_err) = CredentialsBootstrap::try_init().await {
                tracing::debug!(error = %init_err, "Credentials init failed during fallback");
            }
            if let Ok(creds) = CredentialsBootstrap::require()
                && creds.user_email.as_str() != user_email
            {
                return get_or_create_admin(db_pool, creds.user_email.as_str(), context_type).await;
            }
            Err(e)
        },
        Err(e) => Err(e),
    }
}

pub(super) async fn resolve_tenant_admin_with_fallback(
    db_pool: &DbPool,
    creds: &CloudCredentials,
    user_email: &str,
    session_email_hint: Option<&str>,
) -> Result<systemprompt_users::User> {
    match get_or_create_admin(db_pool, user_email, "tenant").await {
        Ok(user) => Ok(user),
        Err(e) if session_email_hint.is_some() && creds.user_email.as_str() != user_email => {
            tracing::warn!(
                email = %user_email,
                error = %e,
                "Session user lookup failed, falling back to cloud credentials"
            );
            get_or_create_admin(db_pool, creds.user_email.as_str(), "tenant").await
        },
        Err(e) => Err(e),
    }
}
