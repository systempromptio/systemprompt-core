use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_cloud::{CredentialsBootstrap, ProfilePath};
use systemprompt_core_database::{Database, DbPool};
use systemprompt_core_logging::CliService;
use systemprompt_core_security::{AdminTokenParams, JwtService};
use systemprompt_core_tui::services::cloud_api::create_tui_session;
use systemprompt_core_tui::{CloudParams, TuiApp};
use systemprompt_core_users::{User, UserService};
use systemprompt_identifiers::JwtToken;
use systemprompt_models::{ApiPaths, Config, SecretsBootstrap};

use crate::cloud::deploy_select::{discover_profiles, DiscoveredProfile};

async fn check_local_api(api_url: &str) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let health_url = format!("{}{}", api_url.trim_end_matches('/'), ApiPaths::HEALTH);

    match client.get(&health_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                Ok(())
            } else {
                Err(format!(
                    "API returned status {}: server may be misconfigured",
                    response.status()
                ))
            }
        },
        Err(e) => {
            if e.is_connect() {
                Err("Connection refused - server is not running".to_string())
            } else if e.is_timeout() {
                Err("Connection timed out - server may be starting up".to_string())
            } else {
                Err(format!("Connection failed: {}", e))
            }
        },
    }
}

fn prompt_profile_selection(profiles: &[DiscoveredProfile]) -> Result<usize> {
    if profiles.is_empty() {
        anyhow::bail!(
            "No profiles found.\n\nCreate a profile with: systemprompt cloud profile create <name>"
        );
    }

    if profiles.len() == 1 {
        CliService::info(&format!("Using profile: {}", profiles[0].name));
        return Ok(0);
    }

    let options: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a profile")
        .items(&options)
        .default(0)
        .interact()
        .context("Failed to get profile selection")?;

    Ok(selection)
}

async fn fetch_admin_user_by_email(database_url: &str, email: &str) -> Result<User> {
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
                "User '{}' not found in database.\n\nRun 'systemprompt cloud login' to sync your \
                 user to all profiles.",
                email
            )
        })?;

    if !user.is_admin() {
        anyhow::bail!(
            "User '{}' is not an admin.\n\nRun 'systemprompt cloud login' to sync your admin role \
             to all profiles.",
            email
        );
    }

    Ok(user)
}

pub async fn execute() -> Result<()> {
    CredentialsBootstrap::try_init()?;

    let creds = CredentialsBootstrap::require()
        .with_context(|| {
            "Not logged in to SystemPrompt Cloud.\n\nRun 'systemprompt cloud login' to \
             authenticate first."
        })?
        .clone();

    CliService::info("Authenticating with SystemPrompt Cloud...");
    if let Some(ref email) = creds.user_email {
        CliService::key_value("User", email);
    }

    if !creds.validate_with_api().await.unwrap_or(false) {
        anyhow::bail!(
            "Cloud token is no longer valid.\n\nRun 'systemprompt cloud login' to re-authenticate."
        );
    }
    CliService::success("Token valid");

    let profiles: Vec<_> = discover_profiles()
        .context("Failed to discover profiles")?
        .into_iter()
        .filter(|p| p.profile.database.external_db_access || p.profile.target.is_local())
        .collect();

    CliService::section("Available profiles");
    for p in &profiles {
        CliService::info(&format!("  - {}", p.name));
    }

    let selected_idx = prompt_profile_selection(&profiles)?;
    let selected = &profiles[selected_idx];

    selected
        .profile
        .validate()
        .with_context(|| format!("Failed to validate profile: {}", selected.name))?;

    let profile = selected.profile.clone();

    CliService::info("Checking local API server...");
    CliService::key_value("URL", &profile.server.api_external_url);

    if let Err(reason) = check_local_api(&profile.server.api_external_url).await {
        if profile.database.external_db_access {
            anyhow::bail!(
                "Cloud instance is not accessible.\n\nURL: {}\nError: {}\n\nThe cloud server may \
                 be stopped or unreachable.\nCheck your cloud deployment status with: \
                 systemprompt cloud status",
                profile.server.api_external_url,
                reason
            );
        }
        anyhow::bail!(
            "Local API server is not accessible.\n\nURL: {}\nError: {}\n\nPlease start the local \
             server first:\n  cd {}\n  just serve",
            profile.server.api_external_url,
            reason,
            profile.paths.system
        );
    }
    CliService::success("Status: running");

    let cloud_email = creds.user_email.as_ref().ok_or_else(|| {
        anyhow::anyhow!("No email in cloud credentials. Run 'systemprompt cloud login'.")
    })?;

    CliService::info("Loading admin user...");

    let profile_dir = selected
        .path
        .parent()
        .context("Invalid profile path - no parent directory")?;
    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);
    let secrets_content = std::fs::read_to_string(&secrets_path)
        .with_context(|| format!("Failed to read secrets from {}", secrets_path.display()))?;
    let secrets: serde_json::Value =
        serde_json::from_str(&secrets_content).context("Failed to parse profile secrets.json")?;
    let database_url = secrets
        .get("database_url")
        .and_then(|v| v.as_str())
        .context("No database_url in profile secrets.json")?;

    let selected_admin = fetch_admin_user_by_email(database_url, cloud_email)
        .await
        .context("Failed to fetch admin user from database")?;

    CliService::info("Creating TUI session...");
    let session_id = create_tui_session(
        &profile.server.api_external_url,
        selected_admin.id.as_str(),
        &selected_admin.email,
    )
    .await
    .context("Failed to create TUI session")?;

    let config = Config::get()?;
    let local_token = JwtService::generate_admin_token(&AdminTokenParams {
        user_id: &selected_admin.id,
        session_id: &session_id,
        email: &selected_admin.email,
        jwt_secret: SecretsBootstrap::jwt_secret()?,
        issuer: &config.jwt_issuer,
        duration: ChronoDuration::hours(24),
    })
    .context("Failed to generate admin token")?;

    CliService::section("Starting TUI");
    CliService::key_value(
        "Profile",
        &format!("{} ({})", profile.display_name, selected.name),
    );
    CliService::key_value("Admin", &selected_admin.email);
    CliService::key_value("Session", session_id.as_str());

    let tenant_id = profile.cloud.as_ref().and_then(|c| c.tenant_id.clone());

    let mut app = TuiApp::new_cloud(CloudParams {
        cloud_api_url: creds.api_url.clone(),
        cloud_token: JwtToken::new(creds.api_token.clone()),
        user_email: creds.user_email.clone(),
        tenant_id,
        profile,
        local_token,
        session_id,
    })
    .await
    .context("Failed to initialize TUI application")?;

    app.run().await.context("TUI application error")?;

    Ok(())
}
