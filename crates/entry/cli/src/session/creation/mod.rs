//! CLI session creation for local and cloud-tenant profiles.
//!
//! Resolves the user, mints a token and records session context. Local sessions
//! resolve the configured administrator by username without provisioning a
//! user; cloud and tenant sessions resolve identities from credential email
//! addresses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod admin_bootstrap;

use anyhow::{Context, Result};
use systemprompt_cloud::{CliSession, CloudCredentials, SessionKey};
use systemprompt_loader::BootContext;
use systemprompt_logging::CliService;
use systemprompt_manifest::Profile;

use super::api::{create_local_session_row, load_signing_keys};
use super::contexts::SessionContexts;
use super::resolution::ProfileContext;
use admin_bootstrap::{
    SessionComponents, build_cli_session, connect_database, generate_admin_token, load_secrets,
    resolve_admin_with_fallback, resolve_credentialed_user_email, resolve_local_admin,
    resolve_tenant_admin_with_fallback,
};
use systemprompt_extension::Admitted;
use systemprompt_extension::cli::CliConfig;


pub(super) async fn create_local_session(
    profile_ctx: &ProfileContext<'_>,
    session_key: &SessionKey,
    config: &CliConfig,
    session_email_hint: Option<&str>,
    source: SessionSource<'_>,
) -> Result<CliSession> {
    let SessionSource {
        boot,
        contexts,
        admitted,
    } = source;
    let profile = boot.profile();
    profile
        .validate()
        .with_context(|| format!("Failed to validate profile: {}", profile_ctx.name))?;

    let secrets = load_secrets(boot);

    if config.is_interactive() {
        CliService::info("Creating local CLI session...");
        CliService::key_value("Profile", profile_ctx.name.as_str());
    }

    let db_pool = connect_database(&secrets).await?;
    let admins = systemprompt_runtime::admin_bootstrap(
        admitted,
        std::sync::Arc::<systemprompt_database::Database>::clone(&db_pool),
    )?;

    let admin_user = if profile.is_local_trial() && session_email_hint.is_none() {
        resolve_local_admin(admins.as_ref(), &profile.system_admin.username).await?
    } else {
        let user_email = resolve_credentialed_user_email(session_email_hint).await?;
        resolve_admin_with_fallback(
            admins.as_ref(),
            user_email.as_str(),
            session_email_hint,
            "local",
        )
        .await?
    };

    if config.is_interactive() {
        CliService::key_value("User", &admin_user.email);
    }

    let keys = load_signing_keys(boot)?;
    let session_id = create_local_session_row(
        admins.as_ref(),
        &admin_user.id,
        chrono::Duration::hours(crate::session::api::DEFAULT_CLI_SESSION_HOURS),
    )
    .await
    .context("Failed to create local CLI session row in the database")?;

    let context_id = contexts
        .issue(&admin_user.id, &session_id, profile_ctx.name)
        .await?;
    let session_token =
        generate_admin_token(&profile.security.issuer, keys, &admin_user, &session_id)?;

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

#[derive(Clone, Copy)]
pub(super) struct SessionSource<'a> {
    pub boot: &'a BootContext,
    pub contexts: &'a SessionContexts,
    pub admitted: &'a Admitted,
}

pub(super) struct TenantSessionParams<'a> {
    pub creds: &'a CloudCredentials,
    pub profile: &'a Profile,
    pub profile_ctx: &'a ProfileContext<'a>,
    pub session_key: &'a SessionKey,
    pub config: &'a CliConfig,
    pub session_email_hint: Option<&'a str>,
    pub boot: &'a BootContext,
    pub contexts: &'a SessionContexts,
    pub admitted: &'a Admitted,
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
        boot,
        contexts,
        admitted,
    } = params;
    profile
        .validate()
        .with_context(|| format!("Failed to validate profile: {}", profile_ctx.name))?;

    let user_email = session_email_hint.unwrap_or(creds.user_email.as_str());
    let secrets = load_secrets(boot);

    if config.is_interactive() {
        CliService::info("Creating CLI session...");
        CliService::key_value("Profile", profile_ctx.name.as_str());
        CliService::key_value("User", user_email);
    }

    let db_pool = connect_database(&secrets).await?;
    let admins = systemprompt_runtime::admin_bootstrap(
        admitted,
        std::sync::Arc::<systemprompt_database::Database>::clone(&db_pool),
    )?;
    let admin_user =
        resolve_tenant_admin_with_fallback(admins.as_ref(), creds, user_email, session_email_hint)
            .await?;

    let keys = load_signing_keys(boot)?;
    let session_id = create_local_session_row(
        admins.as_ref(),
        &admin_user.id,
        chrono::Duration::hours(crate::session::api::DEFAULT_CLI_SESSION_HOURS),
    )
    .await
    .context("Failed to create local tenant CLI session row in the database")?;

    let context_id = contexts
        .issue(&admin_user.id, &session_id, profile_ctx.name)
        .await?;
    let session_token =
        generate_admin_token(&profile.security.issuer, keys, &admin_user, &session_id)?;

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
