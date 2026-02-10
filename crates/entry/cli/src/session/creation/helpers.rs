use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cloud::{CliSession, CloudCredentials, CredentialsBootstrap, SessionKey};
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken};
use systemprompt_models::auth::{Permission, RateLimitTier, UserType};
use systemprompt_models::SecretsBootstrap;
use systemprompt_security::{SessionGenerator, SessionParams};
use systemprompt_users::UserService;

use crate::session::resolution::ProfileContext;

pub(super) struct ResolvedSecrets {
    pub database_url: String,
    pub jwt_secret: String,
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
        jwt_secret: secrets.jwt_secret.clone(),
    })
}

pub(super) async fn connect_database(url: &str) -> Result<DbPool> {
    let db = Database::new_postgres(url)
        .await
        .context("Failed to connect to database")?;
    Ok(DbPool::from(Arc::new(db)))
}

pub(super) async fn get_or_create_admin(
    db_pool: &DbPool,
    email: &str,
    context_type: &str,
) -> Result<systemprompt_users::User> {
    let user_service = UserService::new(db_pool)?;

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
            .assign_roles(&user.id, &["admin".to_string()])
            .await
            .context("Failed to assign admin role to existing user");
    }

    let name = email.split('@').next().unwrap_or("admin").to_string();

    tracing::info!(email = %email, name = %name, context = %context_type, "Auto-provisioning user");

    let user = user_service
        .create(&name, email, None, None)
        .await
        .with_context(|| format!("Failed to create user in {} database", context_type))?;

    user_service
        .assign_roles(&user.id, &["admin".to_string()])
        .await
        .context("Failed to assign admin role to new user")
}

pub(super) fn generate_admin_token(
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

pub(super) async fn create_cli_context(
    db_pool: DbPool,
    user: &systemprompt_users::User,
    session_id: &SessionId,
    profile_name: &str,
) -> Result<ContextId> {
    let context_repo = ContextRepository::new(&db_pool)?;
    context_repo
        .create_context(
            &user.id,
            Some(session_id),
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
) -> Result<CliSession> {
    let profile_name = ProfileName::try_new(profile_ctx.name)
        .map_err(|e| anyhow::anyhow!("Invalid profile name: {}", e))?;
    let email =
        Email::try_new(&admin_user.email).map_err(|e| anyhow::anyhow!("Invalid email: {}", e))?;

    Ok(CliSession::builder(
        profile_name,
        components.session_token,
        components.session_id,
        components.context_id,
    )
    .with_session_key(session_key)
    .with_profile_path(profile_ctx.path.clone())
    .with_user(admin_user.id.clone(), email)
    .with_user_type(UserType::Admin)
    .build())
}

pub(super) async fn resolve_local_user_email(session_email_hint: Option<&str>) -> Result<String> {
    if let Some(email) = session_email_hint {
        return Ok(email.to_string());
    }

    CredentialsBootstrap::try_init()
        .await
        .context("Failed to initialize credentials. Run 'systemprompt cloud auth login'.")?;

    let creds = CredentialsBootstrap::require().map_err(|_| {
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
            if let Ok(creds) = CredentialsBootstrap::require() {
                if creds.user_email != user_email {
                    return get_or_create_admin(db_pool, &creds.user_email, context_type).await;
                }
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
        Err(e) if session_email_hint.is_some() && creds.user_email != user_email => {
            tracing::warn!(
                email = %user_email,
                error = %e,
                "Session user lookup failed, falling back to cloud credentials"
            );
            get_or_create_admin(db_pool, &creds.user_email, "tenant").await
        },
        Err(e) => Err(e),
    }
}
