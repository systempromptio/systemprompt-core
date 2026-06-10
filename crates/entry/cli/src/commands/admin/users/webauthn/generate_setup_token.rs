use anyhow::Result;
use chrono::{Duration, Utc};
use clap::Args as ClapArgs;
use systemprompt_oauth::repository::{CreateSetupTokenParams, OAuthRepository, SetupTokenPurpose};
use systemprompt_oauth::services::webauthn::generate_setup_token;

use crate::commands::admin::users::types::WebauthnSetupTokenOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, ClapArgs)]
pub struct Args {
    #[arg(long, help = "Email of the user to generate token for")]
    pub email: String,

    #[arg(long, default_value = "15", help = "Token validity in minutes")]
    pub expires_minutes: u32,
}

pub(super) async fn execute(args: Args, ctx: &CommandContext) -> Result<CommandOutput> {
    let app = ctx.app_context().await?;
    let oauth_repo = OAuthRepository::new(app.db_pool())?;

    let user = oauth_repo
        .find_user_by_email(&args.email)
        .await?
        .ok_or_else(|| anyhow::anyhow!("User not found: {}", args.email))?;

    let (raw_token, token_hash) = generate_setup_token();
    let expires_at = Utc::now() + Duration::minutes(i64::from(args.expires_minutes));

    let params = CreateSetupTokenParams {
        user_id: user.id,
        token_hash,
        purpose: SetupTokenPurpose::CredentialLink,
        expires_at,
    };

    oauth_repo.store_setup_token(params).await?;

    let external_url = app.config().api_external_url.clone();

    let link_url = format!("{}/auth/link-passkey?token={}", external_url, raw_token);

    let output = WebauthnSetupTokenOutput {
        user_email: args.email.clone(),
        token: raw_token,
        registration_url: link_url,
        expires_minutes: args.expires_minutes,
    };

    Ok(CommandOutput::copy_paste_titled(
        "WebAuthn Setup Token",
        output.registration_url,
    ))
}
