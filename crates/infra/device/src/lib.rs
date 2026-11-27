//! `systemprompt-device` — device identity and trust.
//!
//! Sign-in, keyrings, the configuration and policy stores, manifest fetch,
//! verification, acceptance and apply, the ledgers and the platform modules of
//! a governed device.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod protocol;
pub mod secure;
pub mod client;
pub mod accounts;
pub mod view;
