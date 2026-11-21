//! The device-credential consumer surface the bridge reports installation
//! evidence through.
//!
//! Only the routes a bridge calls are served: device enrollment, the host
//! installation plan (bundle), installation receipts and session bindings.
//! Every request authenticates with an enrolled device's `sp_device_`
//! credential; the consumer identity is derived from it, never accepted from
//! JSON. Administration of managed resources happens in-process through the
//! marketplace repository, not over HTTP.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod consumer;
pub mod contract;
pub mod origin;
