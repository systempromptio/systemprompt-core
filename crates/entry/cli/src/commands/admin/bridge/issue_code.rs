//! `admin bridge issue-code` command minting a device-link code.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_oauth::repository::OAuthRepositories;
use systemprompt_oauth::services::issue_bridge_exchange_code;
use systemprompt_users::{UserRepository, UserService};

use crate::shared::CommandOutput;
use super::types::ExchangeCodeIssuedOutput;
use systemprompt_extension::cli::CommandHost;

#[derive(Debug, Args)]
pub struct IssueCodeArgs {
    #[arg(
        long = "user-id",
        value_name = "USER_ID",
        help = "User id, email, or name to issue the exchange code for"
    )]
    pub user_ref: String,
}

pub(super) async fn execute(args: IssueCodeArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let pool = systemprompt_database::pool_from_handle(host.db().await?)?;
    let oauth = crate::commands::admin::state::get::<Arc<OAuthRepositories>>(host).await?;

    let user_id = super::resolve_user_id(&pool, &args.user_ref).await?;

    let users = UserService::new(Arc::new(UserRepository::new(&pool)));
    let issued = issue_bridge_exchange_code(&oauth.oauth, &users, &user_id).await?;

    let output = ExchangeCodeIssuedOutput {
        user_id: user_id.clone(),
        code: issued.code.clone(),
        expires_at: issued.expires_at,
        message: format!(
            "Exchange code issued for {user_id}; valid until {}",
            issued.expires_at
        ),
    };

    Ok(CommandOutput::card_value("Bridge Exchange Code", &output))
}
