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
//! kept — reported and warned about, not deleted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use chrono::NaiveDate;
use systemprompt_models::services::{
    DiscoveryReport, ProviderEntry, VertexRateCard, VertexRateCardEntry,
};

/// Whether this provider already declares the entry's id or one of its
/// aliases, under any of its declared models.
fn already_declared(provider: &ProviderEntry, entry: &VertexRateCardEntry) -> bool {
    std::iter::once(entry.id.as_str())
        .chain(entry.aliases.iter().map(|alias| alias.as_str()))
        .any(|name| provider.find_model(name).is_some())
}

/// Publish one priced, documented-as-supported model on `today`, or record
/// why it was not: the catalog already spoke for it, or the documentation has
/// withdrawn it.
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

/// Record a serverless model the rate card does not price.
pub fn record_unpriced(upstream: String, report: &mut DiscoveryReport) {
    if !report.discovered_unpriced.contains(&upstream) {
        report.discovered_unpriced.push(upstream);
    }
}

/// Record every rate-card entry for this provider that no listing returned.
pub fn record_unseen(
    card: &VertexRateCard,
    provider: &str,
    seen: &HashSet<String>,
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
