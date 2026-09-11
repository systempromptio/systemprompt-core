//! Folding a classified listing into the provider registry.
//!
//! Two rules carry the whole module. An explicit declaration always wins: the
//! catalog is hand-written and reviewed, discovery is a convenience, and a
//! convenience that silently rewrites a reviewed price is worse than one that
//! adds nothing. And discovery only ever adds: a model the rate card prices
//! but the listing did not return is reported, never removed, because a gap in
//! Google's catalog is Google's editorial decision and not our deprecation.
//!
//! One rule sits above both: a model the documentation no longer supports is
//! not published however Vertex lists it. Retirement is announced on the
//! model page months ahead and the listing never reflects it, so the rate
//! card's lifecycle fields decide, against today's date, with no call to the
//! model. An explicit declaration past that line is the operator's and is
//! kept — reported and warned about, not deleted. "Already declared" means
//! the provider lists the entry's id or any of its aliases under any of its
//! declared models.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::hash::BuildHasher;

use chrono::NaiveDate;
use systemprompt_identifiers::ModelId;
use systemprompt_models::services::{
    DiscoveryReport, ProviderEntry, VertexRateCard, VertexRateCardEntry,
};

fn already_declared(provider: &ProviderEntry, entry: &VertexRateCardEntry) -> bool {
    std::iter::once(entry.id.as_str())
        .chain(entry.aliases.iter().map(ModelId::as_str))
        .any(|name| provider.find_model(name).is_some())
}

pub fn publish(
    provider: &mut ProviderEntry,
    entry: &VertexRateCardEntry,
    today: NaiveDate,
    report: &mut DiscoveryReport,
) {
    let id = entry.id.as_str().to_owned();
    let declared = already_declared(provider, entry);
    if !entry.is_supported(today) {
        if declared {
            tracing::warn!(
                model = %id,
                retires_on = ?entry.retires_on,
                docs = %entry.docs,
                "explicitly declared Vertex model is retiring or unsupported; it stays served \
                 because the catalog declares it, but it should be removed"
            );
        }
        report.retiring.push(id);
        return;
    }
    if declared {
        report.explicit_wins.push(id);
        return;
    }
    provider.models.push(entry.to_provider_model());
    report.discovered_priced.push(id);
}

pub fn record_unpriced(upstream: String, report: &mut DiscoveryReport) {
    if !report.discovered_unpriced.contains(&upstream) {
        report.discovered_unpriced.push(upstream);
    }
}

pub fn record_unseen<S: BuildHasher>(
    card: &VertexRateCard,
    provider: &str,
    seen: &HashSet<String, S>,
    report: &mut DiscoveryReport,
) {
    for entry in card.entries_for(provider) {
        if !seen.contains(&entry.upstream) {
            report
                .priced_not_published
                .push(entry.id.as_str().to_owned());
        }
    }
}
