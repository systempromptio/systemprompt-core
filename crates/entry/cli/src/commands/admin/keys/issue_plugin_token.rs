use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use systemprompt_cloud::CredentialsBootstrap;
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_database::{Database, DbPool};
use systemprompt_logging::CliService;
use systemprompt_models::auth::{AuthenticatedUser, JwtAudience, Permission};
use systemprompt_oauth::services::generation::{
    JwtConfig, JwtSigningParams, generate_access_token_jti, generate_jwt,
};
use systemprompt_users::UserService;

use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct IssuePluginTokenArgs {
    #[arg(
        long,
        env = "SYSTEMPROMPT_ADMIN_EMAIL",
        help = "Admin email to mint the token for. Defaults to the active credentials profile."
    )]
    pub email: Option<String>,

    #[arg(
        long,
        default_value = "cowork-bundle",
        help = "Plugin identifier to embed in the token's `plugin_id` claim."
    )]
    pub plugin_id: String,

    #[arg(
        long,
        default_value = "365",
        help = "Token lifetime in days (1..=365)."
    )]
    pub duration_days: u32,

    #[arg(long, help = "Print only the token (for scripting)")]
    pub token_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct IssuePluginTokenOutput {
    pub plugin_id: String,
    pub email: String,
    pub expires_in_days: u32,
    pub jti: String,
    pub token: String,
}

pub(super) async fn execute(
    args: IssuePluginTokenArgs,
) -> Result<CommandResult<IssuePluginTokenOutput>> {
    let profile = ProfileBootstrap::get().context("No profile loaded")?;
    let secrets = SecretsBootstrap::get().context("Secrets not initialized")?;

    if args.duration_days == 0 || args.duration_days > 365 {
        anyhow::bail!(
            "--duration-days must be between 1 and 365 (got {})",
            args.duration_days
        );
    }

    let email = match args.email.clone() {
        Some(e) => e,
        None => resolve_email().await?,
    };

    let database_url = secrets.effective_database_url(profile.database.external_db_access);
    let db = Database::new_postgres(database_url)
        .await
        .context("Failed to connect to database")?;
    let db_pool = DbPool::from(Arc::new(db));

    let user_service = UserService::new(&db_pool)?;
    let user = user_service
        .find_by_email(&email)
        .await
        .context("Failed to look up admin user")?
        .with_context(|| format!("User '{email}' not found in database"))?;
    if !user.is_admin() {
        anyhow::bail!("User '{}' is not an admin — refusing to mint", email);
    }

    let user_uuid = Uuid::parse_str(user.id.as_str())
        .with_context(|| format!("User id '{}' is not a valid UUID", user.id))?;

    let authenticated = AuthenticatedUser::new_with_roles(
        user_uuid,
        user.name.clone(),
        user.email.clone(),
        vec![Permission::HookGovern, Permission::HookTrack],
        user.roles,
    );

    let signing = JwtSigningParams {
        issuer: &profile.security.issuer,
    };

    let session_id = systemprompt_identifiers::SessionId::generate();
    let jti = generate_access_token_jti();

    let expires_in_hours = i64::from(args.duration_days) * 24;
    let config = JwtConfig {
        permissions: vec![Permission::HookGovern, Permission::HookTrack],
        audience: vec![JwtAudience::Hook],
        expires_in_hours: Some(expires_in_hours),
        resource: Some("plugin".to_owned()),
        plugin_id: Some(args.plugin_id.clone()),
    };

    let token = generate_jwt(&authenticated, config, jti.clone(), &session_id, &signing)
        .context("Failed to mint plugin-scope JWT")?;

    let output = IssuePluginTokenOutput {
        plugin_id: args.plugin_id,
        email,
        expires_in_days: args.duration_days,
        jti,
        token: token.clone(),
    };

    if args.token_only {
        CliService::output(&token);
        return Ok(CommandResult::text(output).with_skip_render());
    }

    Ok(CommandResult::card(output).with_title("Plugin-scope JWT"))
}

async fn resolve_email() -> Result<String> {
    CredentialsBootstrap::try_init()
        .await
        .context("Failed to initialize credentials")?;
    let creds = CredentialsBootstrap::require().map_err(|_e| {
        anyhow::anyhow!(
            "No --email provided and no credentials available. Pass --email or run `systemprompt \
             cloud auth login` first."
        )
    })?;
    Ok(creds.user_email.clone())
}
