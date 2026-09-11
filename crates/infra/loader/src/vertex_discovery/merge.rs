//! Folding a classified listing into the provider registry.
//!
//! Two rules carry the whole module. An explicit declaration always wins: the
//! catalog is hand-written and reviewed, discovery is a convenience, and a
//! convenience that silently rewrites a reviewed price is worse than one that
//! adds nothing. And discovery only ever adds: a model the rate card prices
//! but the listing did not return is reported, never removed, because a gap in
//! Google's catalog is Google's editorial decision and not our deprecation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

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

/// Publish one priced model, or record that the catalog already spoke for it.
pub fn publish(
    provider: &mut ProviderEntry,
    entry: &VertexRateCardEntry,
    report: &mut DiscoveryReport,
) {
    if already_declared(provider, entry) {
        report.explicit_wins.push(entry.id.as_str().to_owned());
        return;
    }
    provider.models.push(entry.to_provider_model());
    report.discovered_priced.push(entry.id.as_str().to_owned());
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
