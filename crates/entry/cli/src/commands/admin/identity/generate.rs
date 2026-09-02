//! `admin identity generate`: emit a fresh identity bundle.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_logging::CliService;

use crate::context::CommandContext;
use crate::shared::generate_identity;

#[derive(Debug, Args)]
pub struct GenerateArgs {
    #[arg(long, help = "Print the bundle as a JSON fragment for a secrets file")]
    pub json: bool,
}

pub(super) fn execute(args: GenerateArgs, ctx: &CommandContext) -> Result<()> {
    let bundle = generate_identity()?;
    if args.json || ctx.cli.is_json_output() {
        let fragment = serde_json::json!({
            "oauth_at_rest_pepper": bundle.oauth_at_rest_pepper,
            "manifest_signing_secret_seed": bundle.manifest_signing_secret_seed,
            "signing_key_pem": bundle.signing_key_pem,
        });
        CliService::output(
            &serde_json::to_string_pretty(&fragment).context("serialising identity bundle")?,
        );
        return Ok(());
    }
    CliService::output(&format!(
        "oauth_at_rest_pepper={}",
        bundle.oauth_at_rest_pepper
    ));
    CliService::output(&format!(
        "manifest_signing_secret_seed={}",
        bundle.manifest_signing_secret_seed
    ));
    CliService::output(&format!("signing_key_pem={}", bundle.signing_key_pem));
    CliService::output(&format!("kid: {}", bundle.signing_kid));
    Ok(())
}
