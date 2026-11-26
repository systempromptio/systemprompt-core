//! User resolution for CLI session creation.
//!
//! Local administrator lookup uses the configured username. Credentialed
//! paths resolve email addresses from session hints or cloud credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::cloud_access::CloudScope;
use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use systemprompt_database::{Database, DbPool, PoolConfig};
use systemprompt_extension::cli::CloudAccount;
use systemprompt_identifiers::{ContextId, Email, SessionId, SessionToken};
use systemprompt_loader::BootContext;
use systemprompt_loader::workspace::{CliSession, SessionBinding, SessionIdentity, SessionKey};
use systemprompt_models::auth::{Permission, RateLimitTier, UserType};
use systemprompt_security::keys::SigningKeys;
use systemprompt_security::{SessionGenerator, SessionParams};
use systemprompt_traits::{AdminAccount, AdminBootstrap, AdminGrant};

use crate::session::resolution::ProfileContext;

pub(super) struct ResolvedSecrets {
    pub database_url: String,
    pub database_write_url: Option<String>,
}

pub(super) fn load_secrets(boot: &BootContext) -> ResolvedSecrets {
    let secrets = boot.secrets().get();
    ResolvedSecrets {
        database_url: secrets.database_url.clone(),
        database_write_url: secrets.database_write_url.clone(),
    }
}

pub(super) async fn connect_database(secrets: &ResolvedSecrets) -> Result<DbPool> {
    let db = Database::connect(
        &secrets.database_url,
        secrets.database_write_url.as_deref(),
        &PoolConfig::default(),
    )
    .await
    .context("Failed to connect to database")?;
    Ok(DbPool::from(Arc::new(db)))
}

pub async fn get_or_create_admin(
    admins: &dyn AdminBootstrap,
    email: &str,
    context_type: &str,
) -> Result<AdminAccount> {
    let email = Email::try_new(email).map_err(|e| {
        anyhow::anyhow!("refusing to provision an admin for an invalid address: {e}")
    })?;
    let email = email.as_str();

    if let Some(user) = admins
        .find_by_email(email)
        .await
        .context("Failed to query user by email")?
    {
        if user.service_account {
            anyhow::bail!("{email} is a service account; it cannot open a CLI session");
        }
        if user.is_admin() {
            return Ok(user);
        }

        tracing::info!(email = %email, context = %context_type, "Promoting existing user to admin");

        return admins
            .grant_admin(&user.id, AdminGrant::Only)
            .await
            .context("Failed to assign admin role to existing user");
    }

    let name = email.split('@').next().unwrap_or("admin").to_owned();

    tracing::info!(email = %email, name = %name, context = %context_type, "Auto-provisioning user");

    let user = match admins
        .create_if_absent(&name, email)
        .await
        .with_context(|| format!("Failed to create user in {context_type} database"))?
    {
        Some(user) => user,
        None => admins
            .find_by_email(email)
            .await
            .context("Failed to query user by email")?
            .with_context(|| format!("User {email} vanished between provisioning and lookup"))?,
    };

    admins
        .grant_admin(&user.id, AdminGrant::Only)
        .await
        .context("Failed to assign admin role to new user")
}

pub fn generate_admin_token(
    issuer: &str,
    keys: SigningKeys,
    user: &AdminAccount,
    session_id: &SessionId,
) -> Result<SessionToken> {
    let generator = SessionGenerator::new(issuer, keys);
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

pub(super) struct SessionComponents {
    pub session_token: SessionToken,
    pub session_id: SessionId,
    pub context_id: ContextId,
}

pub(super) fn build_cli_session(
    profile_ctx: &ProfileContext<'_>,
    session_key: &SessionKey,
    components: SessionComponents,
    admin_user: &AdminAccount,
    issuer: &str,
) -> Result<CliSession> {
    let email =
        Email::try_new(&admin_user.email).map_err(|e| anyhow::anyhow!("Invalid email: {}", e))?;

    Ok(CliSession::builder(
        SessionBinding::new(profile_ctx.name.clone(), issuer.to_owned()),
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
    admins: &dyn AdminBootstrap,
    admin_name: &str,
) -> Result<AdminAccount> {
    let user = admins
        .find_by_name(admin_name)
        .await
        .context("Failed to query the local admin user by name")?
        .with_context(|| {
            format!(
                "Local admin user '{admin_name}' not found.\n\nRun 'systemprompt admin bootstrap \
                 --email <your email>' to create it with a real address."
            )
        })?;

    if !user.active {
        anyhow::bail!("Local admin user '{admin_name}' exists but is not active.");
    }
    if user.service_account {
        anyhow::bail!("User '{admin_name}' is a service account; it cannot open a CLI session.");
    }
    if !user.is_admin() {
        anyhow::bail!(
            "User '{admin_name}' exists but does not hold the admin role. Run 'systemprompt admin \
             bootstrap' to repair it."
        );
    }

    Ok(user)
}

#[doc(hidden)]
pub async fn resolve_credentialed_user_email(
    session_email_hint: Option<&str>,
    scope: CloudScope<'_>,
) -> Result<Email> {
    if let Some(email) = session_email_hint {
        return Email::try_new(email).context("session email hint is not a valid email address");
    }

    require_cloud_email(scope, "resolving the session user from the cloud account").await
}

pub async fn require_cloud_email(scope: CloudScope<'_>, action: &'static str) -> Result<Email> {
    let cloud = scope.require(action)?;
    match cloud
        .account()
        .await
        .context("Failed to initialize credentials. Run 'systemprompt cloud auth login'.")?
    {
        CloudAccount::SignedIn { user_email, .. } => Ok(user_email),
        CloudAccount::LocalOnly { reason } => Err(anyhow::anyhow!(
            "Cloud authentication required for new sessions ({reason}).\n\nRun 'systemprompt \
             cloud auth login' to authenticate."
        )),
    }
}

#[doc(hidden)]
pub async fn resolve_admin_with_fallback(
    admins: &dyn AdminBootstrap,
    user_email: &str,
    session_email_hint: Option<&str>,
    context_type: &str,
    scope: CloudScope<'_>,
) -> Result<AdminAccount> {
    match get_or_create_admin(admins, user_email, context_type).await {
        Ok(user) => Ok(user),
        Err(e) if session_email_hint.is_some() => {
            tracing::warn!(
                email = %user_email,
                error = %e,
                "Session user lookup failed, falling back to cloud credentials"
            );
            let cloud_email = match require_cloud_email(scope, "falling back to the cloud account")
                .await
            {
                Ok(email) => Some(email),
                Err(init_err) => {
                    tracing::debug!(error = %init_err, "Credentials init failed during fallback");
                    None
                },
            };
            if let Some(cloud_email) = cloud_email
                && cloud_email.as_str() != user_email
            {
                return get_or_create_admin(admins, cloud_email.as_str(), context_type).await;
            }
            Err(e)
        },
        Err(e) => Err(e),
    }
}

pub async fn resolve_tenant_admin_with_fallback(
    admins: &dyn AdminBootstrap,
    cloud_email: &Email,
    user_email: &str,
    session_email_hint: Option<&str>,
) -> Result<AdminAccount> {
    match get_or_create_admin(admins, user_email, "tenant").await {
        Ok(user) => Ok(user),
        Err(e) if session_email_hint.is_some() && cloud_email.as_str() != user_email => {
            tracing::warn!(
                email = %user_email,
                error = %e,
                "Session user lookup failed, falling back to cloud credentials"
            );
            get_or_create_admin(admins, cloud_email.as_str(), "tenant").await
        },
        Err(e) => Err(e),
    }
}
