use anyhow::Result;
use chrono::{Duration, Utc};
use clap::Args as ClapArgs;
use std::sync::Arc;
use systemprompt_oauth::repository::{CreateSetupTokenParams, OAuthRepository, SetupTokenPurpose};
use systemprompt_oauth::services::webauthn::generate_setup_token;
use systemprompt_runtime::AppContext;

use crate::commands::admin::users::types::WebauthnSetupTokenOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, ClapArgs)]
pub struct Args {
    #[arg(long, help = "Email of the user to generate token for")]
    pub email: String,

    #[arg(long, default_value = "15", help = "Token validity in minutes")]
    pub expires_minutes: u32,
}

pub async fn execute(
    args: Args,
    _config: &CliConfig,
) -> Result<CommandResult<WebauthnSetupTokenOutput>> {
    let ctx = AppContext::new().await?;
    let oauth_repo = OAuthRepository::new(Arc::clone(ctx.db_pool()))?;

    let user = oauth_repo
        .find_user_by_email(&args.email)
        .await?
        .ok_or_else(|| anyhow::anyhow!("User not found: {}", args.email))?;

    let (raw_token, token_hash) = generate_setup_token();
    let expires_at = Utc::now() + Duration::minutes(i64::from(args.expires_minutes));

    let params = CreateSetupTokenParams {
        user_id: user.id.to_string(),
        token_hash,
        purpose: SetupTokenPurpose::CredentialLink,
        expires_at,
    };

    oauth_repo.store_setup_token(params).await?;

    let external_url = ctx.config().api_external_url.clone();

    let link_url = format!("{}/auth/link-passkey?token={}", external_url, raw_token);

    let output = WebauthnSetupTokenOutput {
        user_email: args.email.clone(),
        token: raw_token,
        registration_url: link_url,
        expires_minutes: args.expires_minutes,
    };

    Ok(CommandResult::copy_paste(output).with_title("WebAuthn Setup Token"))
}
