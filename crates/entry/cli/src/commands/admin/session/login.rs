//! `admin session login` command minting a local session and token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use clap::Args;
use serde::{Deserialize, Serialize};

use crate::context::CommandContext;
use crate::paths::ResolvedPaths;
use crate::shared::CommandOutput;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cloud::SessionKey;
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{ContextId, SessionId, SessionToken, UserId};
use systemprompt_loader::BootContext;
use systemprompt_logging::CliService;
use systemprompt_manifest::Profile;
use systemprompt_models::auth::{Permission, RateLimitTier, UserType};
use systemprompt_security::keys::SigningKeys;
use systemprompt_security::{SessionGenerator, SessionParams};
use systemprompt_users::{User, UserRole};

use super::login_helpers::{
    SessionStoreParams, fetch_admin_user, save_session_to_store, try_use_existing_session,
};
use crate::session::api::{create_local_session_row, load_signing_keys};

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

pub async fn execute(args: LoginArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    login_for_profile(ctx.boot()?, &args).await
}

fn ensure_interactive_admin(admin_user: &User) -> Result<()> {
    if admin_user.is_service_account() {
        anyhow::bail!(
            "User '{}' is a service account; service accounts cannot open an interactive session",
            admin_user.name
        );
    }
    Ok(())
}

async fn connect_profile_database(boot: &BootContext) -> Result<DbPool> {
    let profile = boot.profile();
    let secrets = boot.secrets().get();
    let database_url = secrets.effective_database_url(profile.database.external_db_access);
    let db = Database::new_postgres(database_url)
        .await
        .context("Failed to connect to database")?;
    Ok(DbPool::from(Arc::new(db)))
}

pub async fn login_for_profile(boot: &BootContext, args: &LoginArgs) -> Result<CommandOutput> {
    let profile = boot.profile();
    let profile_path = boot
        .profile_path()
        .to_str()
        .context("Profile path is not valid UTF-8")?;
    let sessions_dir = ResolvedPaths::discover().sessions_dir();
    let session_key = session_key_for_profile(profile);

    let db_pool = connect_profile_database(boot).await?;

    if !args.force_new
        && let Some(output) = try_use_existing_session(
            &sessions_dir,
            &session_key,
            &profile.security.issuer,
            args,
            &db_pool,
        )
        .await?
    {
        return Ok(output);
    }

    let admin_name = &profile.system_admin.username;
    progress(args, &format!("Fetching admin user: {admin_name}"));
    let admin_user = fetch_admin_user(
        &db_pool,
        admin_name,
        profile.target.is_cloud(),
        args.email.as_deref(),
    )
    .await?;
    ensure_interactive_admin(&admin_user)?;

    progress(args, "Creating session...");
    let keys = load_signing_keys(boot)?;
    let session_id = create_local_session_row(
        &db_pool,
        &admin_user.id,
        chrono::Duration::hours(args.duration_hours),
        keys.clone(),
        boot,
    )
    .await?;

    progress(args, "Creating context...");
    let context_id =
        create_cli_context(&db_pool, &admin_user.id, &session_id, profile_path).await?;

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

async fn create_cli_context(
    db_pool: &DbPool,
    user_id: &UserId,
    session_id: &SessionId,
    profile_path: &str,
) -> Result<ContextId> {
    let profile_name = Path::new(profile_path)
        .parent()
        .and_then(|d| d.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let context_repo = ContextRepository::new(db_pool);
    context_repo
        .get_or_create_cli_context(
            user_id,
            session_id,
            &format!("CLI Session - {}", profile_name),
        )
        .await
        .context("Failed to create CLI context")
}

fn generate_session_token(
    profile: &Profile,
    keys: SigningKeys,
    args: &LoginArgs,
    admin_user: &User,
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
            roles: vec![UserRole::Admin.as_str().to_owned()],
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
