//! The pure diff of a dry run: each fact the replay would have written is
//! compared with the stored fact of the same `(kind, identity_hash)`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::FactKindId;
use systemprompt_models::brain::FactStatus;

/// One fact's identity, statement and status, replayed or stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactState {
    pub kind: FactKindId,
    pub identity_hash: String,
    pub statement_hash: String,
    pub status: FactStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayChange {
    Create,
    Revise,
    Dispute,
    Unchanged,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeCounts {
    pub create: u64,
    pub revise: u64,
    pub dispute: u64,
    pub unchanged: u64,
}

impl ChangeCounts {
    pub const fn record(&mut self, change: ReplayChange) {
        match change {
            ReplayChange::Create => self.create += 1,
            ReplayChange::Revise => self.revise += 1,
            ReplayChange::Dispute => self.dispute += 1,
            ReplayChange::Unchanged => self.unchanged += 1,
        }
    }
}

/// What a dry run would have done to the stored facts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayReport {
    pub pipeline_version: String,
    pub events: u64,
    pub failed: u64,
    pub undecodable: u64,
    pub truncated: bool,
    pub would_create: u64,
    pub would_revise: u64,
    pub would_dispute: u64,
    pub unchanged: u64,
    pub tokens: u64,
    pub per_kind: BTreeMap<FactKindId, ChangeCounts>,
}

#[must_use]
pub fn classify(replayed: &FactState, stored: Option<&FactState>) -> ReplayChange {
    let Some(stored) = stored else {
        return ReplayChange::Create;
    };
    if replayed.status == FactStatus::Disputed && stored.status != FactStatus::Disputed {
        return ReplayChange::Dispute;
    }
    if replayed.statement_hash != stored.statement_hash || replayed.status != stored.status {
        return ReplayChange::Revise;
    }
    ReplayChange::Unchanged
}

pub fn tally(report: &mut ReplayReport, replayed: &[FactState], stored: &[FactState]) {
    let stored: HashMap<(&FactKindId, &str), &FactState> = stored
        .iter()
        .map(|s| ((&s.kind, s.identity_hash.as_str()), s))
        .collect();
    for fact in replayed {
        let change = classify(
            fact,
            stored
                .get(&(&fact.kind, fact.identity_hash.as_str()))
                .copied(),
        );
        report
            .per_kind
            .entry(fact.kind.clone())
            .or_default()
            .record(change);
        match change {
            ReplayChange::Create => report.would_create += 1,
            ReplayChange::Revise => report.would_revise += 1,
            ReplayChange::Dispute => report.would_dispute += 1,
            ReplayChange::Unchanged => report.unchanged += 1,
        }
    }
}
