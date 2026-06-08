use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use clap::Args;
use serde::{Deserialize, Serialize};

use crate::cli_settings::CliConfig;
use crate::paths::ResolvedPaths;
use crate::shared::CommandOutput;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cloud::SessionKey;
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_logging::CliService;
use systemprompt_models::auth::{Permission, RateLimitTier, UserType};
use systemprompt_models::{Profile, Secrets};
use systemprompt_security::{SessionGenerator, SessionParams};
use systemprompt_users::UserRole;

use super::login_helpers::{
    SessionStoreParams, fetch_admin_user, save_session_to_store, try_use_existing_session,
};
use crate::session::api::create_local_session_row;

#[derive(Debug, Args)]
pub struct LoginArgs {
    #[arg(
        long,
        env = "SYSTEMPROMPT_ADMIN_EMAIL",
        hide = true,
        help = "Override email from credentials"
    )]
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

pub async fn execute(args: LoginArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let profile = ProfileBootstrap::get().context("No profile loaded")?;
    let profile_path = ProfileBootstrap::get_path().context("Profile path not set")?;
    let secrets = SecretsBootstrap::get().context("Secrets not initialized")?;

    login_for_profile(profile, profile_path, secrets, &args).await
}

pub async fn login_for_profile(
    profile: &Profile,
    profile_path: &str,
    secrets: &Secrets,
    args: &LoginArgs,
) -> Result<CommandOutput> {
    let sessions_dir = ResolvedPaths::discover().sessions_dir();
    let session_key = session_key_for_profile(profile);

    let database_url = secrets.effective_database_url(profile.database.external_db_access);

    let db = Database::new_postgres(database_url)
        .await
        .context("Failed to connect to database")?;
    let db_pool = DbPool::from(Arc::new(db));

    if !args.force_new {
        if let Some(output) =
            try_use_existing_session(&sessions_dir, &session_key, args, &db_pool).await?
        {
            return Ok(output);
        }
    }

    let admin_name = &profile.system_admin.username;
    if !args.token_only {
        CliService::info(&format!("Fetching admin user: {admin_name}"));
    }
    let admin_user = fetch_admin_user(
        &db_pool,
        admin_name,
        profile.target.is_cloud(),
        args.email.as_deref(),
    )
    .await?;

    if !args.token_only {
        CliService::info("Creating session...");
    }
    let session_id = create_local_session_row(&db_pool, &admin_user.id).await?;

    if !args.token_only {
        CliService::info("Creating context...");
    }
    let profile_name = Path::new(profile_path)
        .parent()
        .and_then(|d| d.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let context_repo = ContextRepository::new(&db_pool)?;
    let context_id = context_repo
        .create_context(
            &admin_user.id,
            Some(&session_id),
            &format!("CLI Session - {}", profile_name),
        )
        .await
        .context("Failed to create CLI context")?;

    if !args.token_only {
        CliService::info("Generating token...");
    }
    let session_generator = SessionGenerator::new(&profile.security.issuer);
    let duration = ChronoDuration::hours(args.duration_hours);
    let session_token = session_generator
        .generate(&SessionParams {
            user_id: &admin_user.id,
            session_id: &session_id,
            email: &admin_user.email,
            duration,
            user_type: UserType::Admin,
            permissions: vec![Permission::Admin],
            roles: vec![UserRole::Admin.as_str().to_owned()],
            attributes: std::collections::BTreeMap::new(),
            rate_limit_tier: RateLimitTier::Admin,
        })
        .context("Failed to generate session token")?;

    save_session_to_store(SessionStoreParams {
        sessions_dir: &sessions_dir,
        session_key: &session_key,
        profile_path,
        session_token: session_token.clone(),
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
        CliService::output(session_token.as_str());
        return Ok(CommandOutput::card_value("Admin Session", &output).with_skip_render());
    }

    CliService::success(&format!(
        "Session saved to {}/index.json",
        sessions_dir.display()
    ));
    Ok(CommandOutput::card_value("Admin Session", &output))
}

fn session_key_for_profile(profile: &Profile) -> SessionKey {
    if profile.target.is_local() {
        SessionKey::Local
    } else {
        let tenant_id = profile.cloud.as_ref().and_then(|c| c.tenant_id.as_ref());
        SessionKey::from_tenant_id(tenant_id)
    }
}
