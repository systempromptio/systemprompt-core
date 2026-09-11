//! Boot-time discovery of the Vertex AI models a deployment can serve.
//!
//! The provider catalog ships with a hand-written list of Vertex models. That
//! list goes stale in one direction only — Google adds models, renames them,
//! and occasionally withdraws one — and every staleness costs the same thing:
//! a model we are entitled to and have priced is simply not on offer.
//!
//! So at boot, for every provider whose secret is a Google service-account key
//! and whose endpoint is a Vertex host, this module lists Model Garden, keeps
//! the entries the rate card prices, and appends the ones the catalog did not
//! already declare. It cannot fail a boot: a listing that 403s, a model that
//! is priced but unlisted, a model listed but unpriced — each becomes a line
//! in [`DiscoveryReport`] and a `warn!`, because none of them is a reason for
//! an instance not to start.
//!
//! The whole run is bounded by the caller's timeout. Discovery is a
//! convenience and boot is not: if Google is slow, the instance starts with
//! the catalog it shipped with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod classify;
pub mod client;
pub mod merge;

use std::collections::HashSet;
use std::time::Duration;

use systemprompt_models::services::{DiscoveryReport, ProviderRegistry, VertexRateCard};
use systemprompt_security::google::{ServiceAccountKey, access_token};

use classify::Classification;

/// Every Vertex host ends this way; an endpoint that does not is some other
/// provider using a Google-shaped credential and is left alone.
const VERTEX_HOST_SUFFIX: &str = "aiplatform.googleapis.com";

/// One provider's discoverable shape, resolved before any network call.
struct Plan {
    index: usize,
    provider: String,
    secret_name: String,
    host: String,
    key: ServiceAccountKey,
    publishers: Vec<String>,
}

/// What one provider's listings produced.
#[derive(Default)]
struct Listing {
    models: Vec<classify::PublisherModel>,
    failures: Vec<String>,
}

/// Append every priced, serverless Vertex model the registry does not already
/// declare, and report everything that did not go that way.
///
/// `secret` resolves a secret name to its value; discovery reads it rather
/// than the secrets store directly so that it stays callable from a test and
/// from a boot path that has already loaded secrets.
pub async fn discover(
    providers: &mut ProviderRegistry,
    secret: &(dyn Fn(&str) -> Option<String> + Sync),
    timeout: Duration,
) -> DiscoveryReport {
    let mut report = DiscoveryReport {
        ran_at: chrono::Utc::now().to_rfc3339(),
        ..DiscoveryReport::default()
    };

    let card = match VertexRateCard::embedded() {
        Ok(card) => card,
        Err(e) => {
            tracing::warn!("vertex discovery skipped: {e}");
            report.failed_publishers.push(format!("rate card: {e}"));
            return report;
        },
    };

    let plans = plan(providers, &card, secret, &mut report);
    if plans.is_empty() {
        return report;
    }

    let http = reqwest::Client::new();
    for plan in plans {
        let listing = match tokio::time::timeout(timeout, list(&http, &plan)).await {
            Ok(listing) => listing,
            Err(_) => {
                let note = format!(
                    "{}: discovery timed out after {}s",
                    plan.provider,
                    timeout.as_secs()
                );
                tracing::warn!("{note}");
                report.failed_publishers.push(note);
                continue;
            },
        };
        for failure in listing.failures {
            tracing::warn!("vertex discovery: {failure}");
            report.failed_publishers.push(failure);
        }
        absorb(providers, &plan, &card, listing.models, &mut report);
    }

    report
}

/// Resolve which registry entries are discoverable, and how.
fn plan(
    providers: &ProviderRegistry,
    card: &VertexRateCard,
    secret: &(dyn Fn(&str) -> Option<String> + Sync),
    report: &mut DiscoveryReport,
) -> Vec<Plan> {
    let mut plans = Vec::new();
    for (index, entry) in providers.providers.iter().enumerate() {
        let publishers = card.publishers_for(entry.name.as_str());
        if publishers.is_empty() {
            continue;
        }
        let Some(host) = vertex_host(&entry.endpoint) else {
            continue;
        };
        let secret_name = entry.api_key_secret.as_str().to_owned();
        let Some(value) = secret(&secret_name) else {
            continue;
        };
        match ServiceAccountKey::parse(&value) {
            Ok(Some(key)) => plans.push(Plan {
                index,
                provider: entry.name.as_str().to_owned(),
                secret_name,
                host,
                key,
                publishers,
            }),
            // Why: a provider keyed with something other than a service
            // account is not a discovery failure — it is an API key, and
            // Vertex is simply not reachable that way. Only a *malformed*
            // service account is worth reporting.
            Ok(None) => {},
            Err(e) => report
                .failed_publishers
                .push(format!("{}: {e}", entry.name.as_str())),
        }
    }
    plans
}

/// The origin of a Vertex endpoint, or `None` if it is not a Vertex host.
fn vertex_host(endpoint: &str) -> Option<String> {
    let url = url::Url::parse(endpoint).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if host != VERTEX_HOST_SUFFIX && !host.ends_with(&format!("-{VERTEX_HOST_SUFFIX}")) {
        return None;
    }
    Some(format!("{}://{host}", url.scheme()))
}

/// Mint a token and list every publisher this provider prices.
async fn list(http: &reqwest::Client, plan: &Plan) -> Listing {
    let mut listing = Listing::default();
    let token = match access_token(&plan.secret_name, &plan.key).await {
        Ok(token) => token,
        Err(e) => {
            listing.failures.push(format!(
                "{}: could not mint an access token: {e}",
                plan.provider
            ));
            return listing;
        },
    };

    let (models, failures) =
        client::list_all(http, &plan.host, &token, &plan.provider, &plan.publishers).await;
    listing.models = models;
    listing.failures.extend(failures);
    listing
}

/// Fold one provider's listing into the registry.
fn absorb(
    providers: &mut ProviderRegistry,
    plan: &Plan,
    card: &VertexRateCard,
    models: Vec<classify::PublisherModel>,
    report: &mut DiscoveryReport,
) {
    let Some(provider) = providers.providers.get_mut(plan.index) else {
        return;
    };
    let mut seen: HashSet<String> = HashSet::new();

    for model in &models {
        let (classification, entry) = classify::classify(model, card, &plan.provider);
        match classification {
            Classification::NotServerless => {},
            Classification::Unpriced => merge::record_unpriced(model.upstream(), report),
            Classification::PreviewWithheld => {
                if let Some(entry) = entry {
                    seen.insert(entry.upstream.clone());
                }
            },
            Classification::Publish => {
                if let Some(entry) = entry {
                    seen.insert(entry.upstream.clone());
                    merge::publish(provider, entry, report);
                }
            },
        }
    }

    merge::record_unseen(card, &plan.provider, &seen, report);
}
