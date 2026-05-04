use std::path::Path;

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use systemprompt_cloud::{CliSession, SessionKey, SessionStore};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ClientId, ContextId, SessionId, UserId};
use systemprompt_logging::CliService;
use systemprompt_users::{User, UserService};

use super::login::{LoginArgs, LoginOutput};
use crate::shared::CommandResult;

#[derive(Debug, Serialize)]
struct SessionRequest {
    client_id: ClientId,
    user_id: UserId,
    email: String,
}

#[derive(Debug, Deserialize)]
struct SessionResponse {
    session_id: SessionId,
}

pub(super) async fn try_use_existing_session(
    sessions_dir: &Path,
    session_key: &SessionKey,
    args: &LoginArgs,
    db_pool: &DbPool,
) -> Result<Option<CommandResult<LoginOutput>>> {
    let mut store = SessionStore::load_or_create(sessions_dir)?;

    let Some(session) = store.get_valid_session(session_key) else {
        if !args.token_only {
            CliService::info("No valid session found, creating new session...");
        }
        return Ok(None);
    };

    let session_id = session.session_id.clone();
    let user_id = session.user_id.clone();
    let user_email = session.user_email.to_string();
    let session_token = session.session_token.clone();

    let user_service = UserService::new(db_pool)?;
    let exists = user_service
        .session_exists(&session_id)
        .await
        .unwrap_or(false);

    if !exists {
        if !args.token_only {
            CliService::info(
                "Cached session is stale (not found in database), creating new session...",
            );
        }
        store.remove_session(session_key);
        store.save(sessions_dir)?;
        return Ok(None);
    }

    let output = LoginOutput {
        status: "existing".to_string(),
        user_id,
        email: user_email,
        session_id,
        expires_in_hours: 24,
    };

    if args.token_only {
        CliService::output(session_token.as_str());
        return Ok(Some(CommandResult::text(output).with_skip_render()));
    }

    CliService::success("Using existing valid session");
    Ok(Some(
        CommandResult::card(output).with_title("Admin Session"),
    ))
}

pub(super) async fn fetch_admin_user(
    db_pool: &DbPool,
    email: &str,
    is_cloud_profile: bool,
) -> Result<User> {
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

pub(super) async fn create_session(
    api_url: &str,
    user_id: &UserId,
    email: &str,
) -> Result<SessionId> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("Failed to create HTTP client")?;

    let url = format!(
        "{}/api/v1/core/oauth/session",
        api_url.trim_end_matches('/')
    );

    let request = SessionRequest {
        client_id: ClientId::new("sp_cli"),
        user_id: user_id.clone(),
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

    Ok(session_response.session_id)
}

pub(super) struct SessionStoreParams<'a> {
    pub sessions_dir: &'a Path,
    pub session_key: &'a SessionKey,
    pub profile_path: &'a str,
    pub session_token: systemprompt_identifiers::SessionToken,
    pub session_id: SessionId,
    pub context_id: ContextId,
    pub user_id: UserId,
    pub user_email: &'a str,
}

pub(super) fn save_session_to_store(params: SessionStoreParams<'_>) -> Result<()> {
    let SessionStoreParams {
        sessions_dir,
        session_key,
        profile_path,
        session_token,
        session_id,
        context_id,
        user_id,
        user_email,
    } = params;
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
