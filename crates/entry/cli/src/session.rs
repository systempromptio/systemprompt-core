use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use serde::{Deserialize, Serialize};
use systemprompt_cloud::paths::{get_cloud_paths, CloudPath};
use systemprompt_cloud::{
    CliSession, CloudCredentials, CredentialsBootstrap, ProfilePath, ProjectContext,
};
use systemprompt_core_agent::repository::context::ContextRepository;
use systemprompt_core_database::{Database, DbPool};
use systemprompt_core_logging::CliService;
use systemprompt_core_security::{SessionGenerator, SessionParams};
use systemprompt_core_users::UserService;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, SessionToken, TraceId, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::Profile;

use crate::CliConfig;

struct ProfileContext<'a> {
    dir: &'a Path,
    name: &'a str,
    path: PathBuf,
}

#[derive(Debug)]
pub struct CliSessionContext {
    pub session: CliSession,
    pub profile: Profile,
}

impl CliSessionContext {
    pub const fn session_token(&self) -> &SessionToken {
        &self.session.session_token
    }

    pub const fn context_id(&self) -> &ContextId {
        &self.session.context_id
    }

    pub fn api_url(&self) -> &str {
        &self.profile.server.api_external_url
    }

    pub fn to_request_context(&self, agent_name: &str) -> RequestContext {
        RequestContext::new(
            self.session.session_id.clone(),
            TraceId::generate(),
            self.session.context_id.clone(),
            AgentName::new(agent_name.to_string()),
        )
        .with_user_id(self.session.user_id.clone())
        .with_auth_token(self.session.session_token.as_str())
        .with_user_type(self.session.user_type)
    }
}

fn try_session_from_env(profile: &Profile) -> Option<CliSessionContext> {
    if std::env::var("SYSTEMPROMPT_CLI_REMOTE").is_err() {
        return None;
    }

    let session_id = std::env::var("SYSTEMPROMPT_SESSION_ID").ok()?;
    let context_id = std::env::var("SYSTEMPROMPT_CONTEXT_ID").ok()?;
    let user_id = std::env::var("SYSTEMPROMPT_USER_ID").ok()?;
    let auth_token = std::env::var("SYSTEMPROMPT_AUTH_TOKEN").ok()?;

    let session = CliSession::builder(
        "remote",
        SessionToken::new(auth_token),
        SessionId::new(session_id),
        ContextId::new(context_id),
    )
    .with_user(UserId::new(user_id), "remote@cli.local")
    .with_user_type(UserType::Admin)
    .build();

    Some(CliSessionContext {
        session,
        profile: profile.clone(),
    })
}

pub async fn get_or_create_session(config: &CliConfig) -> Result<CliSessionContext> {
    let profile = ProfileBootstrap::get()
        .map_err(|_| {
            anyhow::anyhow!(
                "Profile required.\n\nSet SYSTEMPROMPT_PROFILE environment variable to your \
                 profile.yaml path."
            )
        })?
        .clone();

    if let Some(ctx) = try_session_from_env(&profile) {
        return Ok(ctx);
    }

    let profile_path_str = ProfileBootstrap::get_path().map_err(|_| {
        anyhow::anyhow!("Profile path required.\n\nSet SYSTEMPROMPT_PROFILE environment variable.")
    })?;

    let profile_path = Path::new(profile_path_str);
    let profile_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid profile path: no parent directory"))?;
    let profile_name = profile_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid profile directory name"))?
        .to_string();

    let project_ctx = ProjectContext::discover();
    let session_path = if project_ctx.systemprompt_dir().exists() {
        project_ctx.local_session()
    } else {
        get_cloud_paths()
            .context("Failed to resolve cloud paths from profile configuration")?
            .resolve(CloudPath::CliSession)
    };

    if let Ok(session) = CliSession::load_from_path(&session_path) {
        if session.is_valid_for_profile(&profile_name) {
            let mut session = session;
            session.touch();
            session
                .save_to_path(&session_path)
                .context("Failed to update session file")?;
            return Ok(CliSessionContext { session, profile });
        }
    }

    CredentialsBootstrap::try_init()
        .await
        .context("Failed to initialize credentials. Run 'systemprompt cloud auth login'.")?;

    let creds = CredentialsBootstrap::require()
        .map_err(|_| {
            anyhow::anyhow!(
                "Cloud authentication required.\n\nRun 'systemprompt cloud auth login' to \
                 authenticate."
            )
        })?
        .clone();

    let profile_ctx = ProfileContext {
        dir: profile_dir,
        name: &profile_name,
        path: PathBuf::from(profile_path_str),
    };

    let session = create_session_for_profile(&creds, &profile, &profile_ctx, config).await?;

    session
        .save_to_path(&session_path)
        .with_context(|| format!("Failed to save session to {}", session_path.display()))?;

    if !session_path.exists() {
        anyhow::bail!(
            "Session file was not created at {}. Check write permissions.",
            session_path.display()
        );
    }

    if session.session_token.as_str().is_empty() {
        anyhow::bail!("Session token is empty. Session creation failed.");
    }

    Ok(CliSessionContext { session, profile })
}

async fn create_session_for_profile(
    creds: &CloudCredentials,
    profile: &Profile,
    profile_ctx: &ProfileContext<'_>,
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
        })
        .context("Failed to generate session token")?;

    if config.is_interactive() {
        CliService::success("Session created");
        CliService::key_value("Session ID", session_id.as_str());
        CliService::key_value("Context ID", context_id.as_str());
    }

    Ok(
        CliSession::builder(profile_ctx.name, session_token, session_id, context_id)
            .with_profile_path(profile_ctx.path.clone())
            .with_user(admin_user.id, admin_user.email)
            .with_user_type(UserType::Admin)
            .build(),
    )
}

async fn fetch_admin_user(db_pool: &DbPool, email: &str) -> Result<systemprompt_core_users::User> {
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

pub fn clear_session() -> Result<()> {
    let cloud_paths = get_cloud_paths()?;
    let session_path = cloud_paths.resolve(CloudPath::CliSession);
    CliSession::delete_from_path(&session_path)?;
    Ok(())
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
