use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cloud::{CliSession, CloudCredentials, CredentialsBootstrap, SessionKey};
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken};
use systemprompt_logging::CliService;
use systemprompt_models::auth::{Permission, RateLimitTier, UserType};
use systemprompt_models::{Profile, SecretsBootstrap};
use systemprompt_security::{SessionGenerator, SessionParams};
use systemprompt_users::UserService;

use super::api::request_session_id;
use super::resolution::ProfileContext;
use crate::CliConfig;

struct ResolvedSecrets {
    database_url: String,
    jwt_secret: String,
}

enum AdminLookupContext {
    Local,
    Tenant,
}

fn load_secrets() -> Result<ResolvedSecrets> {
    let secrets = SecretsBootstrap::get().map_err(|e| {
        anyhow::anyhow!(
            "Secrets not initialized: {}\n\nEnsure your profile has a valid secrets \
             configuration.\nCheck that secrets.json exists or environment variables are set.",
            e
        )
    })?;

    Ok(ResolvedSecrets {
        database_url: secrets.database_url.clone(),
        jwt_secret: secrets.jwt_secret.clone(),
    })
}

async fn connect_database(url: &str) -> Result<DbPool> {
    let db = Database::new_postgres(url)
        .await
        .context("Failed to connect to database")?;
    Ok(DbPool::from(Arc::new(db)))
}

async fn fetch_admin(
    db_pool: &DbPool,
    email: &str,
    ctx: AdminLookupContext,
) -> Result<systemprompt_users::User> {
    let user_service = UserService::new(db_pool)?;

    let user = user_service
        .find_by_email(email)
        .await
        .context("Failed to query user by email")?
        .ok_or_else(|| match ctx {
            AdminLookupContext::Local => anyhow::anyhow!(
                "User '{}' not found in local database.\n\nEnsure this user exists, or run \
                 'systemprompt admin users create --email {} --admin'.",
                email,
                email
            ),
            AdminLookupContext::Tenant => anyhow::anyhow!(
                "User '{}' not found in database.\n\nRun 'systemprompt cloud auth login' to sync \
                 your user.",
                email
            ),
        })?;

    if !user.is_admin() {
        match ctx {
            AdminLookupContext::Local => anyhow::bail!(
                "User '{}' is not an admin.\n\nGrant admin role with 'systemprompt admin users \
                 set-role {} admin'.",
                email,
                email
            ),
            AdminLookupContext::Tenant => anyhow::bail!(
                "User '{}' is not an admin.\n\nRun 'systemprompt cloud auth login' to sync your \
                 admin role.",
                email
            ),
        }
    }

    Ok(user)
}

fn generate_admin_token(
    jwt_secret: &str,
    issuer: &str,
    user: &systemprompt_users::User,
    session_id: &SessionId,
) -> Result<SessionToken> {
    let generator = SessionGenerator::new(jwt_secret, issuer);
    generator
        .generate(&SessionParams {
            user_id: &user.id,
            session_id,
            email: &user.email,
            duration: ChronoDuration::hours(24),
            user_type: UserType::Admin,
            permissions: vec![Permission::Admin],
            roles: vec!["admin".to_string()],
            rate_limit_tier: RateLimitTier::Admin,
        })
        .context("Failed to generate session token")
}

async fn create_cli_context(
    db_pool: DbPool,
    user: &systemprompt_users::User,
    session_id: &SessionId,
    profile_name: &str,
) -> Result<ContextId> {
    let context_repo = ContextRepository::new(db_pool);
    context_repo
        .create_context(
            &user.id,
            Some(session_id),
            &format!("CLI Session - {}", profile_name),
        )
        .await
        .context("Failed to create CLI context")
}

pub(super) async fn create_local_session(
    profile: &Profile,
    profile_ctx: &ProfileContext<'_>,
    session_key: &SessionKey,
    config: &CliConfig,
) -> Result<CliSession> {
    profile
        .validate()
        .with_context(|| format!("Failed to validate profile: {}", profile_ctx.name))?;

    CredentialsBootstrap::try_init()
        .await
        .context("Failed to initialize credentials. Run 'systemprompt cloud auth login'.")?;

    let creds = CredentialsBootstrap::require().map_err(|_| {
        anyhow::anyhow!(
            "Cloud authentication required.\n\nRun 'systemprompt cloud auth login' to \
             authenticate."
        )
    })?;

    let user_email = creds.user_email.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "No user email in credentials.\n\nRun 'systemprompt cloud auth login' to authenticate."
        )
    })?;

    let secrets = load_secrets().context("Failed to load secrets")?;

    if config.is_interactive() {
        CliService::info("Creating local CLI session...");
        CliService::key_value("Profile", profile_ctx.name);
    }

    let db_pool = connect_database(&secrets.database_url).await?;
    let admin_user = fetch_admin(&db_pool, user_email, AdminLookupContext::Local).await?;

    if config.is_interactive() {
        CliService::key_value("User", &admin_user.email);
    }

    let session_id = request_session_id(
        &profile.server.api_external_url,
        admin_user.id.as_str(),
        &admin_user.email,
    )
    .await
    .context(
        "Failed to create session via API.\n\nEnsure the API server is running, or use \
         'systemprompt admin session login' to create a session manually.",
    )?;
    let context_id =
        create_cli_context(db_pool, &admin_user, &session_id, profile_ctx.name).await?;
    let session_token = generate_admin_token(
        &secrets.jwt_secret,
        &profile.security.issuer,
        &admin_user,
        &session_id,
    )?;

    if config.is_interactive() {
        CliService::success("Local session created");
        CliService::key_value("Session ID", session_id.as_str());
        CliService::key_value("Context ID", context_id.as_str());
    }

    let profile_name = ProfileName::try_new(profile_ctx.name)
        .map_err(|e| anyhow::anyhow!("Invalid profile name: {}", e))?;
    let email =
        Email::try_new(&admin_user.email).map_err(|e| anyhow::anyhow!("Invalid email: {}", e))?;

    Ok(
        CliSession::builder(profile_name, session_token, session_id, context_id)
            .with_session_key(session_key)
            .with_profile_path(profile_ctx.path.clone())
            .with_user(admin_user.id, email)
            .with_user_type(UserType::Admin)
            .build(),
    )
}

pub(super) async fn create_session_for_tenant(
    creds: &CloudCredentials,
    profile: &Profile,
    profile_ctx: &ProfileContext<'_>,
    session_key: &SessionKey,
    config: &CliConfig,
) -> Result<CliSession> {
    profile
        .validate()
        .with_context(|| format!("Failed to validate profile: {}", profile_ctx.name))?;

    let cloud_email = creds.user_email.as_ref().ok_or_else(|| {
        anyhow::anyhow!("No email in cloud credentials. Run 'systemprompt cloud auth login'.")
    })?;

    let secrets = load_secrets().context("Failed to load secrets")?;

    if config.is_interactive() {
        CliService::info("Creating CLI session...");
        CliService::key_value("Profile", profile_ctx.name);
        CliService::key_value("User", cloud_email);
    }

    let db_pool = connect_database(&secrets.database_url).await?;
    let admin_user = fetch_admin(&db_pool, cloud_email, AdminLookupContext::Tenant).await?;

    let session_id = request_session_id(
        &profile.server.api_external_url,
        admin_user.id.as_str(),
        &admin_user.email,
    )
    .await
    .context("Failed to create CLI session via API")?;

    let context_id =
        create_cli_context(db_pool, &admin_user, &session_id, profile_ctx.name).await?;
    let session_token = generate_admin_token(
        &secrets.jwt_secret,
        &profile.security.issuer,
        &admin_user,
        &session_id,
    )?;

    if config.is_interactive() {
        CliService::success("Session created");
        CliService::key_value("Session ID", session_id.as_str());
        CliService::key_value("Context ID", context_id.as_str());
    }

    let profile_name = ProfileName::try_new(profile_ctx.name)
        .map_err(|e| anyhow::anyhow!("Invalid profile name: {}", e))?;
    let email =
        Email::try_new(&admin_user.email).map_err(|e| anyhow::anyhow!("Invalid email: {}", e))?;

    Ok(
        CliSession::builder(profile_name, session_token, session_id, context_id)
            .with_session_key(session_key)
            .with_profile_path(profile_ctx.path.clone())
            .with_user(admin_user.id, email)
            .with_user_type(UserType::Admin)
            .build(),
    )
}
