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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscoveryReport {
    /// Priced models Vertex published that the catalog did not declare; these
    /// were added to the registry.
    pub discovered_priced: Vec<String>,

    /// Serverless models Vertex published that the rate card does not price;
    /// left unpublished.
    pub discovered_unpriced: Vec<String>,

    /// Rate-card entries no listing returned; their explicit declarations, if
    /// any, are untouched.
    pub priced_not_published: Vec<String>,

    /// Rate-card entries whose id the catalog already declared explicitly; the
    /// declaration wins and discovery changed nothing.
    pub explicit_wins: Vec<String>,

    /// Rate-card entries Vertex published but the documentation no longer
    /// supports — retired, retiring within the notice window, or preview
    /// without an opt-in. Withheld from discovery; an explicit declaration is
    /// kept and warned about.
    #[serde(default)]
    pub retiring: Vec<String>,

    /// `provider/publisher: reason` for each listing that could not complete.
    pub failed_publishers: Vec<String>,

    /// RFC 3339 timestamp of the run.
    pub ran_at: String,
}
