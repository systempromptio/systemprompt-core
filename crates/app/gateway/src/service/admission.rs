//! Commits a dispatch's staged admission writes before the upstream call.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{DispatchError, GatewayError};
use crate::audit::GatewayAudit;

pub(super) async fn commit_admission(audit: &GatewayAudit) -> Result<(), DispatchError> {
    let Err(error) = audit.commit_admission().await else {
        return Ok(());
    };
    let error = GatewayError::internal("audit admission failed", error);
    match audit
        .fail("Gateway admission failed before provider dispatch")
        .await
    {
        Ok(()) => Err(DispatchError::Recorded(error)),
        Err(settlement_error) => {
            tracing::error!(%settlement_error, "Could not record failed gateway admission");
            Err(DispatchError::PreAudit(error))
        },
    }
}
