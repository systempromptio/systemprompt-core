use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Duration as ChronoDuration;
use clap::Args;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::types::LoginOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_core_database::{Database, DbPool};
use systemprompt_core_logging::CliService;
use systemprompt_core_security::{SessionGenerator, SessionParams};
use systemprompt_core_users::{User, UserService};
use systemprompt_identifiers::SessionId;
use systemprompt_models::{ProfileBootstrap, SecretsBootstrap};

#[derive(Debug, Args)]
pub struct LoginArgs {
    #[arg(long, env = "SYSTEMPROMPT_ADMIN_EMAIL", help = "Admin email address")]
    pub email: Option<String>,

    #[arg(long, default_value = "24", help = "Session duration in hours")]
    pub duration_hours: i64,

    #[arg(long, help = "Only output the token (for scripting)")]
    pub token_only: bool,
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

    let email = resolve_input(args.email, "email", config, || {
        Err(anyhow::anyhow!(
            "Admin email is required. Use --email or set SYSTEMPROMPT_ADMIN_EMAIL"
        ))
    })?;

    let secrets = SecretsBootstrap::get().context("Secrets not initialized")?;
    let database_url = &secrets.database_url;
    let jwt_secret = &secrets.jwt_secret;

    if !args.token_only {
        CliService::info(&format!("Fetching admin user: {}", email));
    }
    let admin_user = fetch_admin_user(database_url, &email).await?;

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
        })
        .context("Failed to generate session token")?;

    let output = LoginOutput {
        user_id: admin_user.id.to_string(),
        email: admin_user.email.clone(),
        session_id: session_id.to_string(),
        token: session_token.to_string(),
        expires_in_hours: args.duration_hours,
    };

    if args.token_only {
        CliService::output(session_token.as_str());
        return Ok(CommandResult::text(output).with_skip_render());
    }

    CliService::success("Login successful");
    Ok(CommandResult::card(output).with_title("Session Created"))
}

async fn fetch_admin_user(database_url: &str, email: &str) -> Result<User> {
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
                "User '{}' not found in database.\nRun 'systemprompt cloud login' to sync your user.",
                email
            )
        })?;

    if !user.is_admin() {
        anyhow::bail!(
            "User '{}' is not an admin.\nRun 'systemprompt cloud login' to sync admin role.",
            email
        );
    }

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
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Session creation failed ({}): {}", status, body);
    }

    let session_response: SessionResponse = response
        .json()
        .await
        .context("Failed to parse session response")?;

    Ok(SessionId::new(session_response.session_id))
}
