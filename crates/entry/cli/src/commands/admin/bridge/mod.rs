//! `admin bridge` subcommand: operator tools for the bridge helper.
//!
//! Exposes [`BridgeCommands`] for enrolling device-certificate fingerprints,
//! issuing one-shot session exchange codes, listing active bridge sessions,
//! and rotating the ed25519 manifest signing seed.

mod enroll_cert;
mod issue_code;
mod list;
mod rotate_signing_key;
mod types;

use crate::CliConfig;
use crate::shared::render_result;
use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum BridgeCommands {
    #[command(about = "Enroll a device certificate fingerprint for a user")]
    EnrollCert(enroll_cert::EnrollCertArgs),

    #[command(about = "Issue a one-shot session exchange code for the bridge helper")]
    IssueCode(issue_code::IssueCodeArgs),

    #[command(about = "List active bridge sessions (recent heartbeats)")]
    List(list::ListArgs),

    #[command(
        about = "Generate a fresh ed25519 manifest signing seed and persist it to the secrets file"
    )]
    RotateSigningKey(rotate_signing_key::RotateSigningKeyArgs),
}

pub async fn execute(cmd: BridgeCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        BridgeCommands::EnrollCert(args) => {
            let result = enroll_cert::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        BridgeCommands::IssueCode(args) => {
            let result = issue_code::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        BridgeCommands::List(args) => {
            let result = list::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        BridgeCommands::RotateSigningKey(args) => {
            let result = rotate_signing_key::execute(args, config)?;
            render_result(&result);
            Ok(())
        },
    }
}
