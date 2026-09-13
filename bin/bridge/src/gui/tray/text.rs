//! Tray menu and tooltip text derived from the application snapshot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::state::{AppStateSnapshot, GatewayStatus};

pub(super) fn tooltip(snap: &AppStateSnapshot) -> String {
    format!("{}\n{}", format_identity(snap), format_last_sync(snap))
}

pub(super) const fn is_signed_in(snap: &AppStateSnapshot) -> bool {
    snap.pat_present || snap.verified_identity.is_some()
}

pub(super) fn format_identity(snap: &AppStateSnapshot) -> String {
    match &snap.gateway_status {
        GatewayStatus::Unknown | GatewayStatus::Probing => "Checking gateway…".to_owned(),
        GatewayStatus::Unreachable { .. } => "Gateway unreachable".to_owned(),
        GatewayStatus::Reachable { .. } => match snap.verified_identity.as_ref() {
            Some(id) => {
                let label = id
                    .email
                    .as_deref()
                    .or_else(|| {
                        id.user_id
                            .as_ref()
                            .map(systemprompt_identifiers::UserId::as_str)
                    })
                    .unwrap_or("(verified)");
                format!("Signed in as {label}")
            },
            None if snap.pat_present => "PAT stored — verifying…".to_owned(),
            None => "Not signed in".to_owned(),
        },
    }
}

pub(super) fn format_last_sync(snap: &AppStateSnapshot) -> String {
    snap.last_sync_summary.as_deref().map_or_else(
        || "Last sync: never".to_owned(),
        |s| format!("Last sync: {s}"),
    )
}
