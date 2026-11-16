//! `admin bridge` subcommand: operator tools for the bridge helper.
//!
//! Exposes [`BridgeCommands`] for enrolling device-certificate fingerprints,
//! issuing one-shot session exchange codes, listing active bridge sessions,
//! rotating the ed25519 manifest signing seed, and printing its fingerprint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod enroll_cert;
mod issue_code;
mod list;
mod rotate_signing_key;
pub mod signing_key_fingerprint;
mod types;

use crate::kernel::KernelHost;
use anyhow::{Result, anyhow};
use clap::Subcommand;
use crate::shared::CommandOutput;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_users::{UserAdminService, UserRepository, UserService};

pub(super) async fn resolve_user_id(pool: &DbPool, user_ref: &str) -> Result<UserId> {
    let reference = user_ref.trim();
    if reference.is_empty() {
        return Err(anyhow!("user_id cannot be empty"));
    }

    let admin_service =
        UserAdminService::new(UserService::new(Arc::new(UserRepository::new(pool))));
    admin_service
        .find_user(reference)
        .await?
        .map(|user| user.id)
        .ok_or_else(|| anyhow!("no user with id, email, or name '{reference}'"))
}

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

    #[command(
        about = "Print the manifest signing key's pubkey and the fingerprint the bridge shows \
                 before trusting it"
    )]
    SigningKeyFingerprint(signing_key_fingerprint::SigningKeyFingerprintArgs),
}

pub async fn execute(cmd: BridgeCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match cmd {
        BridgeCommands::EnrollCert(args) => {
            let result = enroll_cert::execute(args, host).await?;
            Ok(result)
        },
        BridgeCommands::IssueCode(args) => {
            let result = issue_code::execute(args, host).await?;
            Ok(result)
        },
        BridgeCommands::List(args) => {
            let result = list::execute(args, host).await?;
            Ok(result)
        },
        BridgeCommands::RotateSigningKey(args) => {
            let result = rotate_signing_key::execute(args, host).await?;
            Ok(result)
        },
        BridgeCommands::SigningKeyFingerprint(args) => {
            let result = signing_key_fingerprint::execute(args, host).await?;
            Ok(result)
        },
    }
}
