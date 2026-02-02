mod helpers;

use anyhow::{Context, Result};
use systemprompt_cloud::{CliSession, CloudCredentials, SessionKey};
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use super::api::request_session_id;
use super::resolution::ProfileContext;
use crate::CliConfig;
use helpers::{
    build_cli_session, connect_database, create_cli_context, generate_admin_token, load_secrets,
    resolve_admin_with_fallback, resolve_local_user_email, resolve_tenant_admin_with_fallback,
    SessionComponents,
};

pub(super) async fn create_local_session(
    profile: &Profile,
    profile_ctx: &ProfileContext<'_>,
    session_key: &SessionKey,
    config: &CliConfig,
    session_email_hint: Option<&str>,
) -> Result<CliSession> {
    profile
        .validate()
        .with_context(|| format!("Failed to validate profile: {}", profile_ctx.name))?;

    let user_email = resolve_local_user_email(session_email_hint).await?;
    let secrets = load_secrets().context("Failed to load secrets")?;

    if config.is_interactive() {
        CliService::info("Creating local CLI session...");
        CliService::key_value("Profile", profile_ctx.name);
    }

    let db_pool = connect_database(&secrets.database_url).await?;
    let admin_user =
        resolve_admin_with_fallback(&db_pool, &user_email, session_email_hint, "local").await?;

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

    build_cli_session(
        profile_ctx,
        session_key,
        SessionComponents {
            session_token,
            session_id,
            context_id,
        },
        &admin_user,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn create_session_for_tenant(
    creds: &CloudCredentials,
    profile: &Profile,
    profile_ctx: &ProfileContext<'_>,
    session_key: &SessionKey,
    config: &CliConfig,
    session_email_hint: Option<&str>,
) -> Result<CliSession> {
    profile
        .validate()
        .with_context(|| format!("Failed to validate profile: {}", profile_ctx.name))?;

    let user_email = session_email_hint.unwrap_or(&creds.user_email);
    let secrets = load_secrets().context("Failed to load secrets")?;

    if config.is_interactive() {
        CliService::info("Creating CLI session...");
        CliService::key_value("Profile", profile_ctx.name);
        CliService::key_value("User", user_email);
    }

    let db_pool = connect_database(&secrets.database_url).await?;
    let admin_user =
        resolve_tenant_admin_with_fallback(&db_pool, creds, user_email, session_email_hint).await?;

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

    build_cli_session(
        profile_ctx,
        session_key,
        SessionComponents {
            session_token,
            session_id,
            context_id,
        },
        &admin_user,
    )
}
