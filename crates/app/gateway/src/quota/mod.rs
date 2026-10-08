//! Gateway quota windows: admission reservation, settlement and decisions.
//!
//! Admission reserves one request plus an estimate of the request's tokens and
//! cost in every window (see `reserve`), so in-flight spend counts against
//! the ceilings; the audit settles the reservation to the audited usage on
//! completion and releases it on failure. Subject-resolution faults follow
//! the configured `QuotaFaultMode`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod decision;
mod estimate;
mod reserve;
mod subject;

pub use decision::{QuotaDecision, QuotaDetail, QuotaDimension};
pub use estimate::{QuotaEstimate, estimate};
pub use reserve::{
    QuotaReservation, QuotaSubjects, QuotaUsage, ReserveOutcome, ReserveParams, ReservedWindow,
    precheck_and_reserve, release, settle,
};

use crate::policies::QuotaWindow;

/// The outcome of a settlement write.
///
/// `Faulted` means spend for this request was not counted against any ceiling.
#[derive(Debug)]
pub enum AccountingOutcome {
    Counted,
    Faulted { message: String },
}
