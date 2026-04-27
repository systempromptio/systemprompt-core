mod enroll_cert;
mod issue_code;
mod rotate_signing_key;
mod types;

use crate::CliConfig;
use crate::shared::render_result;
use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum CoworkCommands {
    #[command(about = "Enroll a device certificate fingerprint for a user")]
    EnrollCert(enroll_cert::EnrollCertArgs),

    #[command(about = "Issue a one-shot session exchange code for the cowork helper")]
    IssueCode(issue_code::IssueCodeArgs),

    #[command(
        about = "Generate a fresh ed25519 manifest signing seed and persist it to the secrets file"
    )]
    RotateSigningKey(rotate_signing_key::RotateSigningKeyArgs),
}

pub async fn execute(cmd: CoworkCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        CoworkCommands::EnrollCert(args) => {
            let result = enroll_cert::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        CoworkCommands::IssueCode(args) => {
            let result = issue_code::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        CoworkCommands::RotateSigningKey(args) => {
            let result = rotate_signing_key::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}
