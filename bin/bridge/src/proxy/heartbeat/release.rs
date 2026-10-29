//! Background staging of a gateway-required bridge release.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ValidatedUrl;

use crate::update::automatic::UpdateGate;

pub(super) async fn stage_required_release(
    update_gate: UpdateGate,
    gateway_base: ValidatedUrl,
    bearer: crate::ids::BearerToken,
    http: reqwest::Client,
) {
    use crate::update::automatic::{self, AutomaticError, Outcome, Skipped, Trigger};

    let client = crate::gateway::GatewayClient::new(gateway_base, http);
    match automatic::run(&update_gate, Trigger::Unsupported, &client, &bearer).await {
        Ok(Outcome::Staged { version, .. }) => {
            tracing::info!(%version, "required bridge release staged for the next launch");
        },
        Ok(Outcome::Current)
        | Err(AutomaticError::Skipped(Skipped::NotDue | Skipped::InFlight)) => {},
        Err(AutomaticError::Skipped(skipped @ Skipped::Policy(_))) => {
            tracing::warn!(reason = %skipped, "a newer bridge is required; update manually");
        },
        Err(AutomaticError::Update(e)) => {
            tracing::error!(error = %e, "a newer bridge is required but staging it failed");
        },
    }
}
