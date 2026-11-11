//! The secrets an agent subprocess is granted.
//!
//! An agent receives the credentials of the catalog endpoints that serve its
//! model (its `model:` or the catalog default) and every tool model override,
//! plus the custom secrets its config lists under `secrets:`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_manifest::services::AgentConfig;
use systemprompt_manifest::services::catalog::ResolvedCatalog;

#[must_use]
pub fn model_credential_secrets(agent: &AgentConfig, catalog: &ResolvedCatalog) -> Vec<String> {
    let primary = agent
        .metadata
        .model
        .clone()
        .or_else(|| catalog.default_model().map(ToString::to_string));
    let overrides = agent
        .metadata
        .tool_model_overrides
        .values()
        .flat_map(|tools| tools.values())
        .filter_map(|config| config.model.clone());
    let models: BTreeSet<String> = primary.into_iter().chain(overrides).collect();
    let secrets: BTreeSet<String> = models
        .iter()
        .flat_map(|model| catalog.enabled_hops(model))
        .filter_map(|hop| hop.credential.platform_secret())
        .map(|name| name.as_str().to_owned())
        .collect();
    secrets.into_iter().collect()
}
