//! Analytics reporting lifecycle on the shared durable event outbox.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod rebuild;
mod status;
mod worker;

pub use rebuild::{initialize, rebuild};
pub use status::{ReportingStatus, status};
pub use worker::{process_pending, spawn};
