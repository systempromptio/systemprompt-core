//! Boot-time discovery of the models a deployment can actually serve.
//!
//! The provider catalog ships with a hand-written list of models. That list
//! goes stale in one direction only — an upstream adds models, renames them,
//! and occasionally withdraws one — and every staleness costs the same thing:
//! a model we are entitled to and have priced is simply not on offer.
//!
//! So at boot, every endpoint of an enabled provider whose platform
//! credential's secret parses into a credential some [`CatalogSource`]
//! accepts is asked what it serves; the entries the rate card prices (keyed
//! by the endpoint's wire) and Google's documentation still supports are
//! kept, and the ones the catalog did not already declare are appended as a
//! model plus one deployment on that endpoint. The listing call is
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
//! with the catalog it shipped with. The listing token is minted under the
//! credential's [`TokenCacheKey`] in a cache the run owns.
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

use systemprompt_identifiers::CredentialId;
use systemprompt_manifest::services::catalog::CatalogDocument;
use systemprompt_manifest::services::{DiscoveryReport, VertexRateCard};
use systemprompt_security::credential::{ProviderCredential, TokenCache, TokenCacheKey};

use classify::Classification;
use source::{CatalogListing, CatalogSource, DiscoveryEndpoint};
use vertex::VertexCatalog;

/// Resolves a secret name to its value.
pub type SecretLookup<'a> = &'a (dyn Fn(&str) -> Option<String> + Sync);

#[must_use]
pub fn default_sources(card: VertexRateCard) -> Vec<Box<dyn CatalogSource>> {
    vec![Box::new(VertexCatalog::new(card))]
}

struct Plan<'a> {
    endpoint: DiscoveryEndpoint,
    source: &'a dyn CatalogSource,
    credential: ProviderCredential,
    credential_id: CredentialId,
}

pub async fn discover(
    document: &mut CatalogDocument,
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
            tracing::warn!(error = %e, "Catalog discovery skipped");
            report.failed_publishers.push(format!("rate card: {e}"));
            return report;
        },
    };

    let sources = default_sources(card.clone());
    let catalog = Catalog {
        sources: &sources,
        card: &card,
    };
    discover_with(document, secret, timeout, catalog, &mut report).await;
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
    document: &mut CatalogDocument,
    secret: SecretLookup<'_>,
    timeout: Duration,
    catalog: Catalog<'_>,
    report: &mut DiscoveryReport,
) {
    let plans = plan(document, secret, catalog.sources, report);
    if plans.is_empty() {
        return;
    }

    let http = reqwest::Client::new();
    let tokens = TokenCache::new();
    for plan in plans {
        let name = plan.endpoint.endpoint.to_string();
        let cache_key = TokenCacheKey::platform(&plan.credential_id);
        let auth = match plan.credential.bearer(&tokens, &cache_key).await {
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

        let listing = plan.source.list(&http, &auth, &plan.endpoint, &scope);
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
        absorb(document, &plan.endpoint, catalog.card, listing, report);
    }
}

fn push_failure(report: &mut DiscoveryReport, note: String) {
    tracing::warn!(note = %note, "Catalog discovery publisher failed");
    report.failed_publishers.push(note);
}

fn plan<'a>(
    document: &CatalogDocument,
    secret: SecretLookup<'_>,
    sources: &'a [Box<dyn CatalogSource>],
    report: &mut DiscoveryReport,
) -> Vec<Plan<'a>> {
    let mut plans = Vec::new();
    for (provider_id, provider) in document.providers.iter() {
        if !provider.enabled {
            continue;
        }
        let endpoints: Vec<DiscoveryEndpoint> = provider
            .endpoints
            .iter()
            .map(|(endpoint_id, endpoint)| DiscoveryEndpoint {
                endpoint: systemprompt_manifest::services::catalog::EndpointRef {
                    provider: provider_id.clone(),
                    endpoint: endpoint_id.clone(),
                },
                wire: endpoint.wire,
                url: endpoint.url.clone(),
            })
            .filter(|endpoint| sources.iter().any(|s| s.matches_endpoint(endpoint)))
            .collect();
        if endpoints.is_empty() {
            continue;
        }
        let Some(secret_name) = document
            .credentials
            .get(provider.credential.as_str())
            .filter(|c| !c.kind.is_personal())
            .and_then(|c| c.secret.clone())
        else {
            continue;
        };
        let Some(value) = secret(secret_name.as_str()) else {
            continue;
        };
        let credential = match ProviderCredential::parse(&value) {
            Ok(credential) => credential,
            Err(e) => {
                report
                    .failed_publishers
                    .push(format!("{}: {e}", provider_id.as_str()));
                continue;
            },
        };
        for endpoint in endpoints {
            if let Some(source) = sources.iter().find(|s| s.applies(&endpoint, &credential)) {
                plans.push(Plan {
                    endpoint,
                    source: source.as_ref(),
                    credential: credential.clone(),
                    credential_id: provider.credential.clone(),
                });
            }
        }
    }
    plans
}

fn absorb(
    document: &mut CatalogDocument,
    endpoint: &DiscoveryEndpoint,
    card: &VertexRateCard,
    listing: CatalogListing,
    report: &mut DiscoveryReport,
) {
    for failure in listing.failures {
        push_failure(report, failure);
    }
    let mut seen: HashSet<String> = HashSet::new();
    let today = chrono::Utc::now().date_naive();

    for model in &listing.models {
        let (classification, entry) = classify::classify_discovered(model, card, endpoint.wire);
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
                    merge::publish(document, endpoint, entry, today, report);
                }
            },
        }
    }

    merge::record_unseen(card, endpoint.wire, &seen, report);
}
