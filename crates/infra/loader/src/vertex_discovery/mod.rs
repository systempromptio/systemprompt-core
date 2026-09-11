//! Boot-time discovery of the models a deployment can actually serve.
//!
//! The provider catalog ships with a hand-written list of models. That list
//! goes stale in one direction only — an upstream adds models, renames them,
//! and occasionally withdraws one — and every staleness costs the same thing:
//! a model we are entitled to and have priced is simply not on offer.
//!
//! So at boot, every provider whose secret parses into a credential some
//! [`CatalogSource`] accepts is asked what it serves; the entries the rate
//! card prices and Google's documentation still supports are kept, and the
//! ones the catalog did not already declare are appended. The listing call is
//! the only network traffic: nothing here ever calls a model to probe it.
//! Discovery cannot fail a boot: a listing that 403s, a model that
//! is priced but unlisted, a model listed but unpriced — each becomes a line
//! in [`DiscoveryReport`] and a `warn!`, because none of them is a reason for
//! an instance not to start.
//!
//! The Vertex specifics live in [`vertex`]; this module holds only the policy
//! that applies to any upstream that can be asked for a catalog. The whole run
//! is bounded by the caller's timeout, per provider. Discovery is a
//! convenience and boot is not: if an upstream is slow, the instance starts
//! with the catalog it shipped with.
//!
//! [`default_sources`] is the list compiled into this build — a single element
//! today; the next upstream that can be asked for a catalog is an entry there
//! and nothing else. [`discover_with`] takes an explicit source list so a test
//! can prove a second source is picked up without the build having to ship
//! one. Secrets are read through [`SecretLookup`] rather than the store
//! directly so discovery stays callable from a test and from a boot path that
//! has already loaded them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod classify;
pub mod client;
pub mod merge;
pub mod source;
pub mod vertex;

use std::collections::HashSet;
use std::time::Duration;

use systemprompt_models::services::{DiscoveryReport, ProviderRegistry, VertexRateCard};
use systemprompt_security::credential::ProviderCredential;

use classify::Classification;
use source::{CatalogListing, CatalogSource};
use vertex::VertexCatalog;

/// Resolves a secret name to its value.
pub type SecretLookup<'a> = &'a (dyn Fn(&str) -> Option<String> + Sync);

#[must_use]
pub fn default_sources(card: VertexRateCard) -> Vec<Box<dyn CatalogSource>> {
    vec![Box::new(VertexCatalog::new(card))]
}

struct Plan<'a> {
    index: usize,
    source: &'a dyn CatalogSource,
    credential: ProviderCredential,
    secret_name: String,
}

pub async fn discover(
    providers: &mut ProviderRegistry,
    secret: SecretLookup<'_>,
    timeout: Duration,
) -> DiscoveryReport {
    let mut report = DiscoveryReport {
        ran_at: chrono::Utc::now().to_rfc3339(),
        ..DiscoveryReport::default()
    };

    let card = match VertexRateCard::embedded() {
        Ok(card) => card,
        Err(e) => {
            tracing::warn!("catalog discovery skipped: {e}");
            report.failed_publishers.push(format!("rate card: {e}"));
            return report;
        },
    };

    let sources = default_sources(card.clone());
    let catalog = Catalog {
        sources: &sources,
        card: &card,
    };
    discover_with(providers, secret, timeout, catalog, &mut report).await;
    report
}

/// The sources a discovery run may list and the card that prices what they
/// return.
#[derive(Clone, Copy)]
pub struct Catalog<'a> {
    pub sources: &'a [Box<dyn CatalogSource>],
    pub card: &'a VertexRateCard,
}

impl std::fmt::Debug for Catalog<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Catalog")
            .field("sources", &self.sources.len())
            .field("card_entries", &self.card.entries.len())
            .finish()
    }
}

pub async fn discover_with(
    providers: &mut ProviderRegistry,
    secret: SecretLookup<'_>,
    timeout: Duration,
    catalog: Catalog<'_>,
    report: &mut DiscoveryReport,
) {
    let plans = plan(providers, secret, catalog.sources, report);
    if plans.is_empty() {
        return;
    }

    let http = reqwest::Client::new();
    for plan in plans {
        let Some(provider) = providers.providers.get(plan.index) else {
            continue;
        };
        let name = provider.name.as_str().to_owned();

        let auth = match plan.credential.bearer(&plan.secret_name).await {
            Ok(auth) => auth,
            Err(e) => {
                push_failure(
                    report,
                    format!("{name}: could not mint an access token: {e}"),
                );
                continue;
            },
        };
        let scope = plan.credential.scope();

        let listing = plan.source.list(&http, &auth, provider, &scope);
        let listing = match tokio::time::timeout(timeout, listing).await {
            Ok(Ok(listing)) => listing,
            Ok(Err(e)) => {
                push_failure(report, format!("{name}: {e}"));
                continue;
            },
            Err(_) => {
                push_failure(
                    report,
                    format!("{name}: discovery timed out after {}s", timeout.as_secs()),
                );
                continue;
            },
        };
        absorb(providers, plan.index, catalog.card, listing, report);
    }
}

fn push_failure(report: &mut DiscoveryReport, note: String) {
    tracing::warn!("catalog discovery: {note}");
    report.failed_publishers.push(note);
}

fn plan<'a>(
    providers: &ProviderRegistry,
    secret: SecretLookup<'_>,
    sources: &'a [Box<dyn CatalogSource>],
    report: &mut DiscoveryReport,
) -> Vec<Plan<'a>> {
    let mut plans = Vec::new();
    for (index, entry) in providers.providers.iter().enumerate() {
        // Why: the cheap, credential-free check comes first so that a
        // malformed secret on a provider no source could have listed anyway is
        // not reported as a discovery failure.
        if !sources.iter().any(|s| s.matches_provider(entry)) {
            continue;
        }
        let secret_name = entry.api_key_secret.as_str().to_owned();
        let Some(value) = secret(&secret_name) else {
            continue;
        };
        let credential = match ProviderCredential::parse(&value) {
            Ok(credential) => credential,
            Err(e) => {
                report
                    .failed_publishers
                    .push(format!("{}: {e}", entry.name.as_str()));
                continue;
            },
        };
        if let Some(source) = sources.iter().find(|s| s.applies(entry, &credential)) {
            plans.push(Plan {
                index,
                source: source.as_ref(),
                credential,
                secret_name,
            });
        }
    }
    plans
}

fn absorb(
    providers: &mut ProviderRegistry,
    index: usize,
    card: &VertexRateCard,
    listing: CatalogListing,
    report: &mut DiscoveryReport,
) {
    for failure in listing.failures {
        push_failure(report, failure);
    }
    let Some(provider) = providers.providers.get_mut(index) else {
        return;
    };
    let name = provider.name.as_str().to_owned();
    let name = name.as_str();
    let mut seen: HashSet<String> = HashSet::new();
    let today = chrono::Utc::now().date_naive();

    for model in &listing.models {
        let (classification, entry) = classify::classify_discovered(model, card, name);
        match classification {
            Classification::NotServerless => {},
            Classification::Unpriced => {
                merge::record_unpriced(model.upstream.clone(), report);
            },
            Classification::PreviewWithheld => {
                if let Some(entry) = entry {
                    seen.insert(entry.upstream.clone());
                }
            },
            Classification::Publish => {
                if let Some(entry) = entry {
                    seen.insert(entry.upstream.clone());
                    merge::publish(provider, entry, today, report);
                }
            },
        }
    }

    merge::record_unseen(card, name, &seen, report);
}
