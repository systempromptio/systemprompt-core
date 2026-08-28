//! Creation of CLI sessions for local and cloud-tenant profiles.
//!
//! Resolves an admin user, mints a session token, and records the session row
//! plus context for both the local ([`create_local_session`]) and tenant
//! ([`create_session_for_tenant`]) paths.
//!
//! The local-trial path *resolves* rather than provisions, and does so by
//! `system_admin.username`. It previously looked the admin up by a hardcoded
//! `admin@localhost.dev` and created one on a miss, which turned `users.email`
//! into a key shared with a migration instead of a fact about a person — and
//! surfaced as a fabricated identity on the bridge device-link consent screen.
//! Cloud and tenant paths still key on email, because there the address comes
//! from real credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod helpers;

use anyhow::{Context, Result};
use systemprompt_cloud::{CliSession, CloudCredentials, SessionKey};
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use super::api::create_local_session_row;
use super::resolution::ProfileContext;
use crate::CliConfig;
use helpers::{
    SessionComponents, build_cli_session, connect_database, create_cli_context,
    generate_admin_token, load_secrets, resolve_admin_with_fallback,
    resolve_credentialed_user_email, resolve_local_admin, resolve_tenant_admin_with_fallback,
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

    let secrets = load_secrets().context("Failed to load secrets")?;

    if config.is_interactive() {
        CliService::info("Creating local CLI session...");
        CliService::key_value("Profile", profile_ctx.name);
    }

    let db_pool = connect_database(&secrets).await?;

    // Why: a local-trial install has no credentials to name a user with, so the
    // admin is resolved by `system_admin.username` — the same key the runtime
    // resolves on — rather than by matching a hardcoded email. Anything else here
    // (a session hint, cloud credentials) is a real address and stays email-keyed.
    let admin_user = if profile.is_local_trial() && session_email_hint.is_none() {
        resolve_local_admin(&db_pool, &profile.system_admin.username).await?
    } else {
        let user_email = resolve_credentialed_user_email(session_email_hint).await?;
        resolve_admin_with_fallback(&db_pool, &user_email, session_email_hint, "local").await?
    };

    if config.is_interactive() {
        CliService::key_value("User", &admin_user.email);
    }

    let session_id = create_local_session_row(
        &db_pool,
        &admin_user.id,
        chrono::Duration::hours(crate::session::api::DEFAULT_CLI_SESSION_HOURS),
    )
    .await
    .context("Failed to create local CLI session row in the database")?;

    let context_id =
        create_cli_context(db_pool, &admin_user, &session_id, profile_ctx.name).await?;
    let session_token = generate_admin_token(&profile.security.issuer, &admin_user, &session_id)?;

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
        &profile.security.issuer,
    )
}

pub(super) struct TenantSessionParams<'a> {
    pub creds: &'a CloudCredentials,
    pub profile: &'a Profile,
    pub profile_ctx: &'a ProfileContext<'a>,
    pub session_key: &'a SessionKey,
    pub config: &'a CliConfig,
    pub session_email_hint: Option<&'a str>,
}

pub(super) async fn create_session_for_tenant(
    params: TenantSessionParams<'_>,
) -> Result<CliSession> {
    let TenantSessionParams {
        creds,
        profile,
        profile_ctx,
        session_key,
        config,
        session_email_hint,
    } = params;
    profile
        .validate()
        .with_context(|| format!("Failed to validate profile: {}", profile_ctx.name))?;

    let user_email = session_email_hint.unwrap_or(creds.user_email.as_str());
    let secrets = load_secrets().context("Failed to load secrets")?;

    if config.is_interactive() {
        CliService::info("Creating CLI session...");
        CliService::key_value("Profile", profile_ctx.name);
        CliService::key_value("User", user_email);
    }

    let db_pool = connect_database(&secrets).await?;
    let admin_user =
        resolve_tenant_admin_with_fallback(&db_pool, creds, user_email, session_email_hint).await?;

    let session_id = create_local_session_row(
        &db_pool,
        &admin_user.id,
        chrono::Duration::hours(crate::session::api::DEFAULT_CLI_SESSION_HOURS),
    )
    .await
    .context("Failed to create local tenant CLI session row in the database")?;

    let context_id =
        create_cli_context(db_pool, &admin_user, &session_id, profile_ctx.name).await?;
    let session_token = generate_admin_token(&profile.security.issuer, &admin_user, &session_id)?;

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
        &profile.security.issuer,
    )
}
