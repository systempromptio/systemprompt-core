//! Persistent device-authenticated installation feedback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::feedback::credentials::Enrollment;
use crate::feedback::outbox::{Delivery, Outbox};
use crate::feedback::{FeedbackError, Result};
use std::process::ExitCode;

pub(super) fn enroll(ctx: &crate::context::BridgeContext, args: &[String]) -> ExitCode {
    let result = (|| -> Result<()> {
        let Some(index) = args.iter().position(|arg| arg == "--token-file") else {
            return Err(FeedbackError::EnrollmentRequired);
        };
        let path = args
            .get(index + 1)
            .ok_or(FeedbackError::EnrollmentRequired)?;
        if std::fs::metadata(path)?.len() > 256 {
            return Err(FeedbackError::EnrollmentRequired);
        }
        let secret = zeroize::Zeroizing::new(std::fs::read_to_string(path)?);
        let config = crate::config::load()?;
        let gateway = crate::config::gateway_url_or_default(&config);
        let response = ctx.block_on(crate::feedback::transport::enroll(
            gateway.as_str(),
            secret.trim(),
        ))?;
        let root = crate::feedback::metadata_root()?;
        let mut enrollment = Enrollment::new(
            gateway.as_str(),
            response.device_id,
            response.consumer_id,
            crate::ids::BearerToken::new(secret.trim().to_owned()),
        )?;
        if let Some(previous) = crate::feedback::enrol::previous_enrolment(&root, gateway.as_str())?
            && previous.consumer_id == enrollment.consumer_id
            && previous.device_id == enrollment.device_id
        {
            enrollment.installation_id = previous.installation_id;
        }
        enrollment.save(&root)?;
        crate::stdio::print_line(&format!(
            "Device {} enrolled for installation feedback",
            enrollment.device_id
        ));
        Ok(())
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            crate::stdio::diag(&format!(
                "device-enroll: {error}; use --token-file with an administrator-issued device credential"
            ));
            ExitCode::FAILURE
        },
    }
}

pub(super) fn status() -> ExitCode {
    let result = (|| -> Result<()> {
        let config = crate::config::load()?;
        let root = crate::feedback::metadata_root()?;
        let enrollment = Enrollment::load(
            &root,
            crate::config::gateway_url_or_default(&config).as_str(),
        )?;
        let outbox = Outbox::new(
            enrollment.outbox_path(&root),
            crate::feedback::outbox::OutboxScope::from_enrollment(&enrollment),
        );
        let entries = outbox.entries()?;
        let pending = outbox.pending_installations()?;
        crate::stdio::print_line(&format!(
            "installation plans: {} unacknowledged, {} superseded without verified evidence",
            pending
                .iter()
                .filter(|(_, pending)| !pending.superseded)
                .count(),
            pending
                .iter()
                .filter(|(_, pending)| pending.superseded)
                .count()
        ));
        let acknowledged = entries
            .iter()
            .filter(|(_, entry)| matches!(entry.delivery, Delivery::Acknowledged(_)))
            .count();
        let verified = entries
            .iter()
            .filter(|(_, entry)| match &entry.delivery {
                Delivery::Acknowledged(receipt) => receipt.fully_verified,
                _ => false,
            })
            .count();
        crate::stdio::print_line(&format!(
            "installation receipts: {acknowledged} acknowledged, {} unacknowledged, {verified} fully verified",
            entries.len() - acknowledged
        ));
        for (_, entry) in entries {
            crate::stdio::print_line(&format!(
                "{:?} {} generation {}: {:?}",
                entry.request.host,
                entry.request.resource_id,
                entry.request.generation,
                entry.delivery
            ));
        }
        Ok(())
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            crate::stdio::diag(&format!("feedback-status: {error}"));
            ExitCode::FAILURE
        },
    }
}
