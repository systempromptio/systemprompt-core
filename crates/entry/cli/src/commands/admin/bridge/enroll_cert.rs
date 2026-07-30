//! `admin bridge enroll-cert` command registering a device-certificate
//! fingerprint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use systemprompt_identifiers::UserId;
use systemprompt_users::{DeviceCertService, EnrollDeviceCertServiceParams};

use super::types::DeviceCertEnrolledOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct EnrollCertArgs {
    #[arg(long, help = "User id, email, or name to enroll the cert for")]
    pub user_id: UserId,

    #[arg(long, help = "SHA-256 fingerprint of the device certificate (hex)")]
    pub fingerprint: String,

    #[arg(
        long,
        help = "Human-readable label for the cert",
        default_value = "device"
    )]
    pub label: String,
}

pub(super) async fn execute(args: EnrollCertArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let app = ctx.app_context().await?;
    let service = DeviceCertService::new(app.db_pool())?;

    let user_id = super::resolve_user_id(app.db_pool(), &args.user_id).await?;

    let record = service
        .enroll(EnrollDeviceCertServiceParams {
            user_id: &user_id,
            fingerprint: &args.fingerprint,
            label: &args.label,
        })
        .await?;

    let output = DeviceCertEnrolledOutput {
        id: record.id.clone(),
        user_id: record.user_id.clone(),
        fingerprint: record.fingerprint.clone(),
        label: record.label.clone(),
        message: format!(
            "Enrolled cert {} for user {}",
            record.fingerprint, record.user_id
        ),
    };

    Ok(CommandOutput::card_value("Device Cert Enrolled", &output))
}
