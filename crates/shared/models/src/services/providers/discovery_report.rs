//! What boot-time Vertex discovery did, in a shape an operator can read.
//!
//! Discovery never fails a boot: a listing that 403s, a model that is priced
//! but no longer published, a model published but unpriced — each is an entry
//! here rather than an error, because none of them is a reason to refuse to
//! start. The report is the only place they surface, so it names things
//! precisely enough to act on: `publisher/model` for upstream names, and
//! `provider/publisher: reason` for a listing that did not complete.
//!
//! `retiring` is `#[serde(default)]` so a report written by an older binary
//! still reads; the CLI and the scheduler treat an absent bucket as empty.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// The buckets of one discovery run.
///
/// `discovered_priced`: priced models Vertex published that the catalog did
/// not declare — added to the registry. `discovered_unpriced`: published but
/// not on the rate card — left unpublished. `priced_not_published`: rate-card
/// entries no listing returned — explicit declarations untouched.
/// `explicit_wins`: the catalog already declared the id, so discovery changed
/// nothing. `retiring`: published but no longer supported by the documentation
/// (retired, inside the notice window, or preview without opt-in) — withheld,
/// with an explicit declaration kept and warned about. `failed_publishers`:
/// `provider/publisher: reason` per listing that did not complete. `ran_at`
/// is the RFC 3339 timestamp of the run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscoveryReport {
    pub discovered_priced: Vec<String>,

    pub discovered_unpriced: Vec<String>,

    pub priced_not_published: Vec<String>,

    pub explicit_wins: Vec<String>,

    #[serde(default)]
    pub retiring: Vec<String>,

    pub failed_publishers: Vec<String>,

    pub ran_at: String,
}
