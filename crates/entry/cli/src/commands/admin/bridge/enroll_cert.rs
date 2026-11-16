//! `admin bridge enroll-cert` command registering a device-certificate
//! fingerprint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_users::{DeviceCertService, EnrollDeviceCertServiceParams, UserRepository};

use crate::shared::CommandOutput;
use super::types::DeviceCertEnrolledOutput;
use systemprompt_extension::cli::CommandHost;

#[derive(Debug, Args)]
pub struct EnrollCertArgs {
    #[arg(
        long = "user-id",
        value_name = "USER_ID",
        help = "User id, email, or name to enroll the cert for"
    )]
    pub user_ref: String,

    #[arg(long, help = "SHA-256 fingerprint of the device certificate (hex)")]
    pub fingerprint: String,

    #[arg(
        long,
        help = "Human-readable label for the cert",
        default_value = "device"
    )]
    pub label: String,
}

pub(super) async fn execute(args: EnrollCertArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let pool = systemprompt_database::pool_from_handle(host.db().await?)?;
    let service = DeviceCertService::new(Arc::new(UserRepository::new(&pool)));

    let user_id = super::resolve_user_id(&pool, &args.user_ref).await?;

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
