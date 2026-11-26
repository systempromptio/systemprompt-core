//! `admin session login` command minting a local session and token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use clap::Args;
use serde::{Deserialize, Serialize};

use systemprompt_database::Database;
use systemprompt_extension::cli::CommandOutput;
use systemprompt_identifiers::{SessionId, SessionToken, UserId};
use systemprompt_loader::BootContext;
use systemprompt_loader::workspace::SessionKey;
use systemprompt_loader::workspace::paths::ResolvedPaths;
use systemprompt_logging::CliService;
use systemprompt_manifest::Profile;
use systemprompt_models::auth::{Permission, RateLimitTier, UserType};
use systemprompt_security::keys::SigningKeys;
use systemprompt_security::{SessionGenerator, SessionParams};
use systemprompt_traits::{AdminAccount, DynAdminBootstrap};

use super::login_helpers::{
    SessionStoreParams, fetch_admin_user, save_session_to_store, try_use_existing_session,
};
use crate::session::SessionContexts;
use crate::session::api::load_signing_keys;
use crate::session::resolution::extract_profile_name;

#[derive(Debug, Args)]
pub struct LoginArgs {
    #[arg(long, hide = true, help = "Override email from credentials")]
    pub email: Option<String>,

    #[arg(long, default_value = "24", help = "Session duration in hours")]
    pub duration_hours: i64,

    #[arg(long, help = "Only output the token (for scripting)")]
    pub token_only: bool,

    #[arg(
        long,
        help = "Force creation of a new session even if a valid one exists"
    )]
    pub force_new: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginOutput {
    pub status: String,
    pub user_id: UserId,
    pub email: String,
    pub session_id: SessionId,
    pub expires_in_hours: i64,
}

pub async fn execute(args: LoginArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    login_for_profile(
        crate::commands::admin::state::boot(host)?.as_ref(),
        &args,
        host,
    )
    .await
}

fn ensure_interactive_admin(admin_user: &AdminAccount) -> Result<()> {
    if admin_user.service_account {
        anyhow::bail!(
            "User '{}' is a service account; service accounts cannot open an interactive session",
            admin_user.name
        );
    }
    Ok(())
}

async fn connect_profile_database(
    boot: &BootContext,
    host: &dyn KernelHost,
) -> Result<DynAdminBootstrap> {
    let profile = boot.profile();
    let secrets = boot.secrets().get();
    let database_url = secrets.effective_database_url(profile.database.external_db_access);
    let db = Database::new_postgres(database_url)
        .await
        .context("Failed to connect to database")?;
    Ok(systemprompt_runtime::admin_bootstrap(
        host.admitted()?,
        Arc::new(db),
    )?)
}

pub(crate) async fn login_for_profile(
    boot: &BootContext,
    args: &LoginArgs,
    host: &dyn KernelHost,
) -> Result<CommandOutput> {
    let profile = boot.profile();
    let profile_path = boot
        .profile_path()
        .to_str()
        .context("Profile path is not valid UTF-8")?;
    let sessions_dir = ResolvedPaths::discover().sessions_dir();
    let session_key = session_key_for_profile(profile);

    let admins = connect_profile_database(boot, host).await?;

    if !args.force_new
        && let Some(output) = try_use_existing_session(
            &sessions_dir,
            &session_key,
            &profile.security.issuer,
            args,
            admins.as_ref(),
        )
        .await?
    {
        return Ok(output);
    }

    let admin_name = &profile.system_admin.username;
    progress(args, &format!("Fetching admin user: {admin_name}"));
    let admin_user = fetch_admin_user(
        admins.as_ref(),
        admin_name,
        profile.target.is_cloud(),
        args.email.as_deref(),
        crate::cloud_access::CloudScope {
            cloud: host.cloud_access(),
            profile: Some(profile),
        },
    )
    .await?;
    ensure_interactive_admin(&admin_user)?;

    progress(args, "Creating session...");
    let keys = load_signing_keys(boot)?;
    let session_id = admins
        .open_cli_session(&admin_user.id, chrono::Duration::hours(args.duration_hours))
        .await
        .context("Failed to create the CLI session row")?;

    progress(args, "Creating context...");
    let context_id = issue_login_context(host, profile_path, &admin_user.id, &session_id).await?;

    progress(args, "Generating token...");
    let session_token = generate_session_token(profile, keys, args, &admin_user, &session_id)?;

    save_session_to_store(SessionStoreParams {
        sessions_dir: &sessions_dir,
        session_key: &session_key,
        profile_path,
        session_token: session_token.clone(),
        issuer: &profile.security.issuer,
        ttl: chrono::Duration::hours(args.duration_hours),
        session_id: session_id.clone(),
        context_id,
        user_id: admin_user.id.clone(),
        user_email: &admin_user.email,
        user_type: UserType::Admin,
    })?;

    let output = LoginOutput {
        status: "created".to_owned(),
        user_id: admin_user.id.clone(),
        email: admin_user.email.clone(),
        session_id,
        expires_in_hours: args.duration_hours,
    };

    if args.token_only {
        return Ok(CommandOutput::text(session_token.as_str()));
    }

    CliService::success(&format!(
        "Session saved to {}/index.json",
        sessions_dir.display()
    ));
    Ok(CommandOutput::card_value("Admin Session", &output))
}

fn progress(args: &LoginArgs, message: &str) {
    if !args.token_only {
        CliService::info(message);
    }
}

async fn issue_login_context(
    host: &dyn KernelHost,
    profile_path: &str,
    user_id: &UserId,
    session_id: &SessionId,
) -> Result<systemprompt_identifiers::ContextId> {
    let path = Path::new(profile_path);
    let profile_name = extract_profile_name(path)?;
    Ok(SessionContexts::for_profile(host, path)
        .await?
        .issue(user_id, session_id, &profile_name)
        .await?)
}

fn generate_session_token(
    profile: &Profile,
    keys: SigningKeys,
    args: &LoginArgs,
    admin_user: &AdminAccount,
    session_id: &SessionId,
) -> Result<SessionToken> {
    let session_generator = SessionGenerator::new(&profile.security.issuer, keys);
    session_generator
        .generate(&SessionParams {
            user_id: &admin_user.id,
            session_id,
            email: &admin_user.email,
            duration: ChronoDuration::hours(args.duration_hours),
            user_type: UserType::Admin,
            permissions: vec![Permission::Admin],
            roles: vec![AdminAccount::ADMIN_ROLE.to_owned()],
            attributes: std::collections::BTreeMap::new(),
            rate_limit_tier: RateLimitTier::Admin,
        })
        .context("Failed to generate session token")
}

fn session_key_for_profile(profile: &Profile) -> SessionKey {
    if profile.target.is_local() {
        SessionKey::Local
    } else {
        let tenant_id = profile.cloud.as_ref().and_then(|c| c.tenant_id.as_ref());
        SessionKey::from_tenant_id(tenant_id)
    }
}
