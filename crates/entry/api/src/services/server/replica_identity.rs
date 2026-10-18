//! Boot-time claim of this replica's instance identity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_runtime::AppContext;

// Why: a redeployed host reuses its HOSTNAME, and the session of the process
// it replaced holds the claim until the server's keepalive probes declare it
// dead; waiting past that window tells a vanished holder from a live one.
pub(super) async fn claim(
    ctx: &AppContext,
) -> Result<systemprompt_database::InstanceClaim, systemprompt_database::InstanceClaimError> {
    let window = std::time::Duration::from_secs(systemprompt_database::CLAIM_DEAD_PEER_SECS * 3);
    let deadline = std::time::Instant::now() + window;
    let mut backoff = std::time::Duration::from_secs(1);
    loop {
        match ctx.service_repository().claim_instance().await {
            Err(systemprompt_database::InstanceClaimError::Claimed { instance_id })
                if std::time::Instant::now() + backoff < deadline =>
            {
                tracing::warn!(
                    instance_id = %instance_id,
                    retry_in_ms = backoff.as_millis(),
                    "replica identity held by another session; waiting for a dead holder to be reaped"
                );
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(std::time::Duration::from_secs(8));
            },
            outcome => return outcome,
        }
    }
}
