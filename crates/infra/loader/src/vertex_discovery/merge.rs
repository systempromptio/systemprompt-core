//! Folding a classified listing into the catalog document.
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
//! model. An explicit declaration inside the retirement notice window is the
//! operator's and is kept — reported and warned about, not deleted.
//!
//! Past the retirement date itself the upstream has switched the model off, so
//! a declaration of a retired model stops being advertised (`clients: []`):
//! it leaves every model listing and picker, and the retirement is logged at
//! error so the operator knows to delete the entry from the catalog. It is
//! not removed because removal empties any route that reached it, and a date
//! passing must not stop an instance from starting.
//!
//! "Already declared" means the catalog declares the entry's id, or a model
//! already deployed on the listing endpoint sends the entry's
//! `upstream_model`. A published entry is appended as one model with one
//! deployment on the listing endpoint, advertised to the client families the
//! endpoint's existing models are (the endpoint's own wire family when it has
//! none yet).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::hash::BuildHasher;

use chrono::NaiveDate;
use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::catalog::{CatalogDocument, DeploymentTarget};
use systemprompt_manifest::services::{DiscoveryReport, VertexRateCard, VertexRateCardEntry};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;

use super::source::DiscoveryEndpoint;

fn deployed_on(document: &CatalogDocument, model: &ModelId, endpoint: &DiscoveryEndpoint) -> bool {
    document
        .deployments
        .get(model.as_str())
        .is_some_and(|targets| targets.iter().any(|t| *t.endpoint() == endpoint.endpoint))
}

fn declared_as(
    document: &CatalogDocument,
    endpoint: &DiscoveryEndpoint,
    entry: &VertexRateCardEntry,
) -> Option<ModelId> {
    if document.models.get(entry.id.as_str()).is_some() {
        return Some(entry.id.clone());
    }
    document
        .models
        .iter()
        .find(|(id, model)| {
            model.upstream_name(id) == entry.upstream_model && deployed_on(document, id, endpoint)
        })
        .map(|(id, _)| id.clone())
}

const fn wire_family(wire: WireProtocol) -> ClientFamily {
    match wire {
        WireProtocol::Anthropic => ClientFamily::Claude,
        WireProtocol::OpenAiChat | WireProtocol::OpenAiResponses => ClientFamily::OpenAi,
        WireProtocol::Gemini => ClientFamily::Gemini,
    }
}

fn endpoint_clients(document: &CatalogDocument, endpoint: &DiscoveryEndpoint) -> Vec<ClientFamily> {
    let mut clients: Vec<ClientFamily> = Vec::new();
    for (id, model) in document.models.iter() {
        if deployed_on(document, id, endpoint) {
            for family in &model.clients {
                if !clients.contains(family) {
                    clients.push(*family);
                }
            }
        }
    }
    if clients.is_empty() {
        clients.push(wire_family(endpoint.wire));
    }
    clients
}

pub fn publish(
    document: &mut CatalogDocument,
    endpoint: &DiscoveryEndpoint,
    entry: &VertexRateCardEntry,
    today: NaiveDate,
    report: &mut DiscoveryReport,
) {
    let id = entry.id.as_str().to_owned();
    let declared = declared_as(document, endpoint, entry);
    if !entry.is_supported(today) {
        match declared {
            Some(model) if entry.is_retired(today) => {
                tracing::error!(
                    model = %model,
                    retires_on = ?entry.retires_on,
                    docs = %entry.docs,
                    "declared Vertex model is past its retirement date and no longer exists \
                     upstream; it is no longer advertised. Remove it from the catalog"
                );
                if let Some(declaration) = document.models.models.get_mut(&model) {
                    declaration.clients.clear();
                }
            },
            Some(model) => {
                tracing::warn!(
                    model = %model,
                    retires_on = ?entry.retires_on,
                    docs = %entry.docs,
                    "explicitly declared Vertex model is retiring or unsupported; it stays served \
                     because the catalog declares it, but it should be removed"
                );
            },
            None => {},
        }
        report.retiring.push(id);
        return;
    }
    if declared.is_some() {
        report.explicit_wins.push(id);
        return;
    }
    let clients = endpoint_clients(document, endpoint);
    document
        .models
        .models
        .insert(entry.id.clone(), entry.to_model_entry(clients));
    document.deployments.deployments.insert(
        entry.id.clone(),
        vec![DeploymentTarget::Endpoint(endpoint.endpoint.clone())],
    );
    report.discovered_priced.push(id);
}

pub fn record_unpriced(upstream: String, report: &mut DiscoveryReport) {
    if !report.discovered_unpriced.contains(&upstream) {
        report.discovered_unpriced.push(upstream);
    }
}

pub fn record_unseen<S: BuildHasher>(
    card: &VertexRateCard,
    wire: WireProtocol,
    seen: &HashSet<String, S>,
    report: &mut DiscoveryReport,
) {
    for entry in card.entries_for(wire) {
        if !seen.contains(&entry.upstream) {
            report
                .priced_not_published
                .push(entry.id.as_str().to_owned());
        }
    }
}
