//! Header names that exist only between the bridge and the gateway.
//!
//! Every header the platform propagates between its own processes is named in
//! `systemprompt_identifiers::headers`; the ones here never leave the bridge's
//! own conversation with the gateway (device attestation on a forwarded hook,
//! the rejection reason the loopback proxy answers with, the delivery id a
//! retried hook carries). A header that gains a reader in a server crate moves
//! to `systemprompt_identifiers::headers`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const HOST: &str = "x-systemprompt-host";
pub const DEVICE_CREDENTIAL: &str = "x-systemprompt-device-credential";
pub const REJECTION_REASON: &str = "x-systemprompt-bridge-reason";
pub const INGESTION_EVENT_ID: &str = "x-ingestion-event-id";
