//! The origin an outbox row is stamped with, and whether a relay delivers it.
//!
//! A process routes its own events to its local subscribers before the
//! outbox row is written, so its relay skips rows it originated. The
//! deployment's instance id is shared by every process it starts (the
//! server and each MCP server it spawns), so the origin is that id scoped
//! to the process: a row an MCP server writes is not the server's own and
//! reaches the server's subscribers. Leases and owner rows keep the
//! unscoped instance id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::InstanceId;

#[must_use]
pub fn process_origin(instance: &InstanceId) -> InstanceId {
    scoped_origin(instance, &std::process::id().to_string())
}

#[must_use]
pub fn scoped_origin(instance: &InstanceId, scope: &str) -> InstanceId {
    InstanceId::new(format!("{}#{scope}", instance.as_str()))
}

#[must_use]
pub fn relays_row(own: &InstanceId, origin: &InstanceId, deliver_to_origin: bool) -> bool {
    deliver_to_origin || origin != own
}
