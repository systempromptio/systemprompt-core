use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;
use systemprompt_users::{DeviceCertService, EnrollDeviceCertServiceParams};

use super::types::DeviceCertEnrolledOutput;
use crate::CliConfig;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct EnrollCertArgs {
    #[arg(long, help = "User ID to enroll the cert for")]
    pub user_id: String,

    #[arg(long, help = "SHA-256 fingerprint of the device certificate (hex)")]
    pub fingerprint: String,

    #[arg(long, help = "Human-readable label for the cert", default_value = "device")]
    pub label: String,
}

pub async fn execute(
    args: EnrollCertArgs,
    _config: &CliConfig,
) -> Result<CommandResult<DeviceCertEnrolledOutput>> {
    let ctx = AppContext::new().await?;
    let service = DeviceCertService::new(ctx.db_pool())?;

    let user_id = UserId::new(args.user_id.trim());
    if user_id.as_str().is_empty() {
        return Err(anyhow!("user_id cannot be empty"));
    }

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

    Ok(CommandResult::text(output).with_title("Device Cert Enrolled"))
}
