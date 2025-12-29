use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt::CredentialsBootstrap;
use systemprompt_core_database::{Database, DbPool};
use systemprompt_core_logging::CliService;
use systemprompt_core_security::{AdminTokenParams, JwtService};
use systemprompt_core_tui::services::cloud_api::create_tui_session;
use systemprompt_core_tui::{CloudParams, TuiApp};
use systemprompt_core_users::{User, UserService};
use systemprompt_identifiers::JwtToken;
use systemprompt_loader::ProfileLoader;
use systemprompt_models::{ApiPaths, Config, Profile, SecretsBootstrap};

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

fn find_services_path() -> Result<String> {
    let config = Config::get()?;
    Ok(config.services_path.clone())
}

fn prompt_profile_selection(profiles: &[String]) -> Result<usize> {
    if profiles.is_empty() {
        anyhow::bail!(
            "No profiles found.\n\nCreate a profile in services/profiles/ (e.g., \
             local.profile.yaml)"
        );
    }

    if profiles.len() == 1 {
        CliService::info(&format!("Using profile: {}", profiles[0]));
        return Ok(0);
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a profile")
        .items(profiles)
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

    let services_path = find_services_path()?;
    let services_dir = Path::new(&services_path);

    let profile_names = Profile::list_available(services_dir);
    CliService::section("Available profiles");
    for name in &profile_names {
        CliService::info(&format!("  - {}", name));
    }

    let selected_idx = prompt_profile_selection(&profile_names)?;
    let profile_name = &profile_names[selected_idx];

    let profile = ProfileLoader::load_and_validate(services_dir, profile_name)
        .with_context(|| format!("Failed to load profile: {}", profile_name))?;

    CliService::info("Checking local API server...");
    CliService::key_value("URL", &profile.server.api_external_url);

    if let Err(reason) = check_local_api(&profile.server.api_external_url).await {
        anyhow::bail!(
            "Local API server is not accessible.\n\nURL: {}\nError: {}\n\nPlease start the local \
             server first:\n\x20\x20cd {}\n\x20\x20just serve\n\nOr if using a different profile, \
             ensure the server.api_external_url\nin your profile configuration points to a \
             running server.",
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
    let database_url = SecretsBootstrap::database_url()?;
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
        &format!("{} ({})", profile.display_name, profile_name),
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
