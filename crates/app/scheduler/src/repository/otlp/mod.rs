//! Persistence for the OTLP exporter: the per-signal cursor rows and the
//! audit-trail reads each export batch is built from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod records;
mod state;
mod tail;

pub use records::{GovernanceRow, LedgerRow, LogRow, RequestRow};
pub use state::{OtlpExportState, OtlpExportStateRepository, Watermark};
pub use tail::{BATCH_ROWS, LogTail, OtlpAuditTailRepository, SETTLE};
