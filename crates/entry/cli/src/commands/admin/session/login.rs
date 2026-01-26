use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use clap::Args;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::cli_settings::CliConfig;
use crate::paths::ResolvedPaths;
use crate::interactive::resolve_required;
use crate::shared::CommandResult;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cloud::{CliSession, SessionKey, SessionStore};
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{ContextId, SessionId};
use systemprompt_logging::CliService;
use systemprompt_models::auth::{Permission, RateLimitTier, UserType};
use systemprompt_models::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_security::{SessionGenerator, SessionParams};
use systemprompt_users::{User, UserService};

#[derive(Debug, Args)]
pub struct LoginArgs {
    #[arg(long, env = "SYSTEMPROMPT_ADMIN_EMAIL", help = "Admin email address")]
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
    pub user_id: systemprompt_identifiers::UserId,
    pub email: String,
    pub session_id: SessionId,
    pub expires_in_hours: i64,
}

#[derive(Debug, Serialize)]
struct SessionRequest {
    client_id: String,
    user_id: String,
    email: String,
}

#[derive(Debug, Deserialize)]
struct SessionResponse {
    session_id: String,
}

pub async fn execute(args: LoginArgs, config: &CliConfig) -> Result<CommandResult<LoginOutput>> {
    let profile = ProfileBootstrap::get().context("No profile loaded")?;
    let profile_path = ProfileBootstrap::get_path().context("Profile path not set")?;

    let sessions_dir = ResolvedPaths::discover().sessions_dir()?;

    let session_key = if profile.target.is_local() {
        SessionKey::Local
    } else {
        let tenant_id = profile.cloud.as_ref().and_then(|c| c.tenant_id.as_deref());
        SessionKey::from_tenant_id(tenant_id)
    };

    if !args.force_new {
        if let Some(output) = try_use_existing_session(&sessions_dir, &session_key, &args)? {
            return Ok(output);
        }
    }

    let email = resolve_required(args.email, "email", config, || {
        Err(anyhow::anyhow!(
            "Admin email is required. Use --email or set SYSTEMPROMPT_ADMIN_EMAIL"
        ))
    })?;

    let secrets = SecretsBootstrap::get().context("Secrets not initialized")?;
    let database_url = &secrets.database_url;
    let jwt_secret = &secrets.jwt_secret;

    let db = Database::new_postgres(database_url)
        .await
        .context("Failed to connect to database")?;
    let db_pool = DbPool::from(Arc::new(db));

    let is_cloud_profile = profile.target.is_cloud();

    if !args.token_only {
        CliService::info(&format!("Fetching admin user: {}", email));
    }
    let admin_user = fetch_admin_user(&db_pool, &email, is_cloud_profile).await?;

    if !args.token_only {
        CliService::info("Creating session...");
    }
    let session_id = create_session(
        &profile.server.api_external_url,
        admin_user.id.as_str(),
        &admin_user.email,
    )
    .await?;

    if !args.token_only {
        CliService::info("Creating context...");
    }
    let profile_name = Path::new(profile_path)
        .parent()
        .and_then(|d| d.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let context_repo = ContextRepository::new(db_pool);
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
    let session_generator = SessionGenerator::new(jwt_secret, &profile.security.issuer);
    let duration = ChronoDuration::hours(args.duration_hours);
    let session_token = session_generator
        .generate(&SessionParams {
            user_id: &admin_user.id,
            session_id: &session_id,
            email: &admin_user.email,
            duration,
            user_type: UserType::Admin,
            permissions: vec![Permission::Admin],
            roles: vec!["admin".to_string()],
            rate_limit_tier: RateLimitTier::Admin,
        })
        .context("Failed to generate session token")?;

    save_session_to_store(
        &sessions_dir,
        &session_key,
        profile_path,
        session_token.clone(),
        session_id.clone(),
        context_id,
        admin_user.id.clone(),
        &admin_user.email,
    )?;

    let output = LoginOutput {
        status: "created".to_string(),
        user_id: admin_user.id.clone(),
        email: admin_user.email.clone(),
        session_id,
        expires_in_hours: args.duration_hours,
    };

    if args.token_only {
        CliService::output(session_token.as_str());
        return Ok(CommandResult::text(output).with_skip_render());
    }

    CliService::success(&format!(
        "Session saved to {}/index.json",
        sessions_dir.display()
    ));
    Ok(CommandResult::card(output).with_title("Admin Session"))
}

fn try_use_existing_session(
    sessions_dir: &Path,
    session_key: &SessionKey,
    args: &LoginArgs,
) -> Result<Option<CommandResult<LoginOutput>>> {
    let store = SessionStore::load_or_create(sessions_dir)?;

    let Some(session) = store.get_valid_session(session_key) else {
        if !args.token_only {
            CliService::info("No valid session found, creating new session...");
        }
        return Ok(None);
    };

    let output = LoginOutput {
        status: "existing".to_string(),
        user_id: session.user_id.clone(),
        email: session.user_email.to_string(),
        session_id: session.session_id.clone(),
        expires_in_hours: 24,
    };

    if args.token_only {
        CliService::output(session.session_token.as_str());
        return Ok(Some(CommandResult::text(output).with_skip_render()));
    }

    CliService::success("Using existing valid session");
    Ok(Some(
        CommandResult::card(output).with_title("Admin Session"),
    ))
}

async fn fetch_admin_user(db_pool: &DbPool, email: &str, is_cloud_profile: bool) -> Result<User> {
    let user_service = UserService::new(db_pool)?;

    if let Some(user) = user_service
        .find_by_email(email)
        .await
        .context("Failed to fetch user")?
    {
        if !user.is_admin() {
            anyhow::bail!(
                "User '{}' exists but is not an admin. Contact your administrator.",
                email
            );
        }
        return Ok(user);
    }

    if !is_cloud_profile {
        anyhow::bail!(
            "User '{}' not found in database.\nFor local profiles, create the user first.",
            email
        );
    }

    CliService::info(&format!(
        "User '{}' not found, creating admin user for cloud profile...",
        email
    ));

    let user = user_service
        .create(email, email, None, None)
        .await
        .context("Failed to create user")?;

    let user = user_service
        .assign_roles(&user.id, &["admin".to_string()])
        .await
        .context("Failed to assign admin role")?;

    CliService::success(&format!("Created admin user: {}", email));
    Ok(user)
}

async fn create_session(api_url: &str, user_id: &str, email: &str) -> Result<SessionId> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("Failed to create HTTP client")?;

    let url = format!(
        "{}/api/v1/core/oauth/session",
        api_url.trim_end_matches('/')
    );

    let request = SessionRequest {
        client_id: "sp_cli".to_string(),
        user_id: user_id.to_string(),
        email: email.to_string(),
    };

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .context("Failed to send session request")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| String::new());
        anyhow::bail!("Session creation failed ({}): {}", status, body);
    }

    let session_response: SessionResponse = response
        .json()
        .await
        .context("Failed to parse session response")?;

    Ok(SessionId::new(session_response.session_id))
}

#[allow(clippy::too_many_arguments)]
fn save_session_to_store(
    sessions_dir: &Path,
    session_key: &SessionKey,
    profile_path: &str,
    session_token: systemprompt_identifiers::SessionToken,
    session_id: SessionId,
    context_id: ContextId,
    user_id: systemprompt_identifiers::UserId,
    user_email: &str,
) -> Result<()> {
    let mut store = SessionStore::load_or_create(sessions_dir)?;

    let profile_dir = Path::new(profile_path).parent();
    let profile_name_str = profile_dir
        .and_then(|d| d.file_name())
        .and_then(|n| n.to_str())
        .context("Invalid profile path")?;

    let profile_name = systemprompt_identifiers::ProfileName::try_new(profile_name_str)
        .map_err(|e| anyhow::anyhow!("Invalid profile name: {}", e))?;

    let email = systemprompt_identifiers::Email::try_new(user_email)
        .map_err(|e| anyhow::anyhow!("Invalid email: {}", e))?;

    let cli_session = CliSession::builder(profile_name, session_token, session_id, context_id)
        .with_session_key(session_key)
        .with_profile_path(profile_path)
        .with_user(user_id, email)
        .build();

    store.upsert_session(session_key, cli_session);
    store.set_active_with_profile(session_key, profile_name_str);
    store.save(sessions_dir)?;

    tracing::debug!("Session saved to {}/index.json", sessions_dir.display());
    Ok(())
}
