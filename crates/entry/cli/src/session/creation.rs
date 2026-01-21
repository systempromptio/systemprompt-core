use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use serde::{Deserialize, Serialize};
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cloud::{CliSession, CloudCredentials, ProfilePath, SessionKey};
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{Email, ProfileName, SessionId};
use systemprompt_logging::CliService;
use systemprompt_models::auth::{Permission, RateLimitTier, UserType};
use systemprompt_models::Profile;
use systemprompt_security::{SessionGenerator, SessionParams};
use systemprompt_users::UserService;

use super::resolution::ProfileContext;
use crate::CliConfig;

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

    let secrets_path = ProfilePath::Secrets.resolve(profile_ctx.dir);
    let secrets_content = std::fs::read_to_string(&secrets_path)
        .with_context(|| format!("Failed to read secrets from {}", secrets_path.display()))?;
    let secrets: serde_json::Value =
        serde_json::from_str(&secrets_content).context("Failed to parse profile secrets.json")?;

    let database_url = secrets
        .get("database_url")
        .and_then(|v| v.as_str())
        .context("No database_url in profile secrets.json")?;
    let jwt_secret = secrets
        .get("jwt_secret")
        .and_then(|v| v.as_str())
        .context("No jwt_secret in profile secrets.json")?;

    if config.is_interactive() {
        CliService::info("Creating CLI session...");
        CliService::key_value("Profile", profile_ctx.name);
        CliService::key_value("User", cloud_email);
    }

    let db = Database::new_postgres(database_url)
        .await
        .context("Failed to connect to database")?;
    let db_arc = Arc::new(db);
    let db_pool = DbPool::from(db_arc);

    let admin_user = fetch_admin_user(&db_pool, cloud_email).await?;

    let session_id = create_cli_session(
        &profile.server.api_external_url,
        admin_user.id.as_str(),
        &admin_user.email,
    )
    .await
    .context("Failed to create CLI session via API")?;

    let context_repo = ContextRepository::new(db_pool);
    let context_id = context_repo
        .create_context(
            &admin_user.id,
            Some(&session_id),
            &format!("CLI Session - {}", profile_ctx.name),
        )
        .await
        .context("Failed to create CLI context")?;

    let session_generator = SessionGenerator::new(jwt_secret, &profile.security.issuer);
    let session_token = session_generator
        .generate(&SessionParams {
            user_id: &admin_user.id,
            session_id: &session_id,
            email: &admin_user.email,
            duration: ChronoDuration::hours(24),
            user_type: UserType::Admin,
            permissions: vec![Permission::Admin],
            roles: vec!["admin".to_string()],
            rate_limit_tier: RateLimitTier::Admin,
        })
        .context("Failed to generate session token")?;

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

async fn fetch_admin_user(db_pool: &DbPool, email: &str) -> Result<systemprompt_users::User> {
    let user_service = UserService::new(db_pool)?;
    let user = user_service
        .find_by_email(email)
        .await
        .context("Failed to fetch user")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "User '{}' not found in database.\n\nRun 'systemprompt cloud auth login' to sync \
                 your user.",
                email
            )
        })?;

    if !user.is_admin() {
        anyhow::bail!(
            "User '{}' is not an admin.\n\nRun 'systemprompt cloud auth login' to sync your admin \
             role.",
            email
        );
    }

    Ok(user)
}

#[derive(Debug, Serialize)]
struct CliSessionRequest {
    client_id: String,
    user_id: String,
    email: String,
}

#[derive(Debug, Deserialize)]
struct CliSessionResponse {
    session_id: String,
}

async fn create_cli_session(api_url: &str, user_id: &str, email: &str) -> Result<SessionId> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("Failed to create HTTP client")?;

    let url = format!(
        "{}/api/v1/core/oauth/session",
        api_url.trim_end_matches('/')
    );

    let request = CliSessionRequest {
        client_id: "sp_cli".to_string(),
        user_id: user_id.to_string(),
        email: email.to_string(),
    };

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .context("Failed to send session creation request")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<error reading response: {}>", e));
        anyhow::bail!("Session creation failed with status {}: {}", status, body);
    }

    let session_response: CliSessionResponse = response
        .json()
        .await
        .context("Failed to parse session response")?;

    Ok(SessionId::new(session_response.session_id))
}
