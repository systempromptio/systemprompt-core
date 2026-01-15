use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use systemprompt_cloud::paths::{get_cloud_paths, CloudPath};
use systemprompt_cloud::{CliSession, CloudCredentials, CredentialsBootstrap, ProfilePath};
use systemprompt_core_database::{Database, DbPool};
use systemprompt_core_logging::CliService;
use systemprompt_core_security::{SessionGenerator, SessionParams};
use systemprompt_core_tui::services::cloud_api::create_tui_session;
use systemprompt_core_users::UserService;
use systemprompt_identifiers::{SessionToken, UserId};
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::Profile;

use crate::CliConfig;

#[derive(Debug)]
pub struct CliSessionContext {
    pub session: CliSession,
    pub profile: Profile,
}

impl CliSessionContext {
    pub fn session_token(&self) -> &SessionToken {
        &self.session.session_token
    }

    pub fn api_url(&self) -> &str {
        &self.profile.server.api_external_url
    }
}

pub async fn get_or_create_session(config: &CliConfig) -> Result<CliSessionContext> {
    CredentialsBootstrap::try_init()
        .context("Failed to initialize credentials. Run 'systemprompt cloud auth login'.")?;

    let creds = CredentialsBootstrap::require()
        .map_err(|_| {
            anyhow::anyhow!(
                "Cloud authentication required.\n\nRun 'systemprompt cloud auth login' to \
                 authenticate."
            )
        })?
        .clone();

    let profile = ProfileBootstrap::get()
        .map_err(|_| {
            anyhow::anyhow!(
                "Profile required.\n\nSet SYSTEMPROMPT_PROFILE environment variable to your \
                 profile.yaml path."
            )
        })?
        .clone();

    let profile_path_str = ProfileBootstrap::get_path().map_err(|_| {
        anyhow::anyhow!(
            "Profile path required.\n\nSet SYSTEMPROMPT_PROFILE environment variable."
        )
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

    let cloud_paths =
        get_cloud_paths().context("Failed to resolve cloud paths from profile configuration")?;
    let session_path = cloud_paths.resolve(CloudPath::CliSession);

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

    let session =
        create_session_for_profile(&creds, &profile, profile_dir, &profile_name, config).await?;

    session
        .save_to_path(&session_path)
        .context("Failed to save session file")?;

    Ok(CliSessionContext { session, profile })
}

async fn create_session_for_profile(
    creds: &CloudCredentials,
    profile: &Profile,
    profile_dir: &Path,
    profile_name: &str,
    config: &CliConfig,
) -> Result<CliSession> {
    profile
        .validate()
        .with_context(|| format!("Failed to validate profile: {}", profile_name))?;

    let cloud_email = creds.user_email.as_ref().ok_or_else(|| {
        anyhow::anyhow!("No email in cloud credentials. Run 'systemprompt cloud auth login'.")
    })?;

    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);
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
        CliService::key_value("Profile", profile_name);
        CliService::key_value("User", cloud_email);
    }

    let admin_user = fetch_admin_user_by_email(database_url, cloud_email).await?;

    let session_id = create_tui_session(
        &profile.server.api_external_url,
        admin_user.id.as_str(),
        &admin_user.email,
    )
    .await
    .context("Failed to create CLI session via API")?;

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
    }

    Ok(CliSession::new(
        profile_name.to_string(),
        session_token,
        session_id,
        UserId::new(admin_user.id.to_string()),
        admin_user.email,
    ))
}

async fn fetch_admin_user_by_email(
    database_url: &str,
    email: &str,
) -> Result<systemprompt_core_users::User> {
    let db = Database::new_postgres(database_url)
        .await
        .context("Failed to connect to database")?;

    let db_arc = Arc::new(db);
    let db_pool = DbPool::from(db_arc);

    let user_service = UserService::new(&db_pool)?;
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
