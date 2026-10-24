//! The catalog the setup wizard and cloud-init seed a new install with.
//!
//! It is core's embedded default catalog ([`DefaultCatalog`]) narrowed to the
//! vendors whose key was supplied: their credentials and providers, every
//! model whose deployments all sit on them, and the default routes whose alias
//! target survives. `gateway.default_model` is the first model the primary
//! vendor serves, so the seeded tree resolves and boots. Operators reshape it
//! with `admin config credential|provider|endpoint|model|deployment|gateway`
//! or by editing `services/ai/`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::GatewayConfigSpec;
use systemprompt_manifest::services::catalog::{CatalogContext, CatalogDocument, DefaultCatalog};

/// Which vendors to keep, which one leads, and whether the gateway is on.
#[derive(Debug, Clone, Copy)]
pub struct SeedRequest<'a> {
    pub present: &'a [&'a str],
    pub primary: Option<&'a str>,
    pub gateway_enabled: bool,
}

pub fn seed_document(request: SeedRequest<'_>) -> Result<CatalogDocument> {
    let mut document =
        DefaultCatalog::document().context("embedded default catalog is unreadable")?;
    let kept = |provider: &str| request.present.contains(&provider);
    document
        .providers
        .providers
        .retain(|id, _| kept(id.as_str()));
    let credentials: Vec<String> = document
        .providers
        .iter()
        .map(|(_, provider)| provider.credential.as_str().to_owned())
        .collect();
    document
        .credentials
        .credentials
        .retain(|id, _| credentials.iter().any(|c| c == id.as_str()));
    document.deployments.deployments.retain(|_, targets| {
        targets.retain(|t| kept(t.endpoint().provider.as_str()));
        !targets.is_empty()
    });
    let deployed: Vec<ModelId> = document.deployments.deployments.keys().cloned().collect();
    document.models.models.retain(|id, _| deployed.contains(id));
    let default_model = request
        .primary
        .and_then(|primary| first_model_on(&document, primary));
    let gateway = document
        .gateway
        .get_or_insert_with(GatewayConfigSpec::default);
    gateway.enabled = request.gateway_enabled;
    gateway.default_model = default_model;
    let models: Vec<ModelId> = document.models.models.keys().cloned().collect();
    gateway.routes.retain(|route| {
        route
            .alias
            .as_ref()
            .is_none_or(|a| models.contains(&a.model))
    });
    document
        .resolve(&CatalogContext::default())
        .context("the seeded catalog does not resolve")?;
    Ok(document)
}

#[must_use]
pub fn first_model_on(document: &CatalogDocument, provider: &str) -> Option<ModelId> {
    document
        .deployments
        .iter()
        .find(|(_, targets)| {
            targets
                .first()
                .is_some_and(|t| t.endpoint().provider.as_str() == provider)
        })
        .map(|(model, _)| model.clone())
}
