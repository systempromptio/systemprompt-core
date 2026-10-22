//! One catalog document, and the files of a `services/ai` directory merged.
//!
//! A [`CatalogDocument`] is whatever one catalog file declares — any of
//! `credentials:`, `providers:`, `models:`, `deployments:` and `gateway:` —
//! and [`CatalogDocument::merge`] folds several into one, refusing an id
//! declared twice. [`CATALOG_FILES`] are the files of the convention, read by
//! [`CatalogDocument::from_ai_dir`].
//!
//! The pre-0.73 shape (a `providers:` list, `ai.default_provider`,
//! `ai.providers`, a gateway `default_provider`, a route with
//! `model_pattern`/`provider`) is refused by [`detect_legacy_shape`] with the
//! `config migrate` hint, never half-loaded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use super::error::{CatalogDocumentError, CatalogResult};
use super::resolved::{CatalogInput, ResolvedCatalog};
use super::{
    CatalogContext, CredentialRegistry, DeploymentRegistry, ModelRegistry, ProviderRegistry,
};
use crate::services::gateway::GatewayConfigSpec;

pub const CATALOG_FILES: [&str; 5] = [
    "credentials.yaml",
    "providers.yaml",
    "models.yaml",
    "deployments.yaml",
    "gateway.yaml",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogDocument {
    #[serde(default)]
    pub credentials: CredentialRegistry,
    #[serde(default)]
    pub providers: ProviderRegistry,
    #[serde(default)]
    pub models: ModelRegistry,
    #[serde(default)]
    pub deployments: DeploymentRegistry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<GatewayConfigSpec>,
}

/// Names the pre-0.73 construct a document still uses, if any.
#[must_use]
pub fn detect_legacy_shape(document: &Value) -> Option<&'static str> {
    if document.get("providers").is_some_and(Value::is_sequence) {
        return Some("`providers:` as a list of entries");
    }
    if let Some(ai) = document.get("ai") {
        if ai.get("default_provider").is_some() {
            return Some("`ai.default_provider`");
        }
        if ai.get("providers").is_some() {
            return Some("`ai.providers`");
        }
    }
    let gateway = document.get("gateway")?;
    if gateway.get("default_provider").is_some() {
        return Some("`gateway.default_provider`");
    }
    let legacy_route = gateway
        .get("routes")
        .and_then(Value::as_sequence)
        .is_some_and(|routes| {
            routes
                .iter()
                .any(|r| r.get("model_pattern").is_some() || r.get("provider").is_some())
        });
    legacy_route.then_some("a gateway route with `model_pattern`/`provider`")
}

impl CatalogDocument {
    pub fn from_yaml_str(origin: &str, text: &str) -> Result<Self, CatalogDocumentError> {
        let parse = |source| CatalogDocumentError::Parse {
            origin: origin.to_owned(),
            source,
        };
        let value: Value = serde_yaml::from_str(text).map_err(parse)?;
        if let Some(what) = detect_legacy_shape(&value) {
            return Err(CatalogDocumentError::LegacyShape {
                origin: origin.to_owned(),
                what,
            });
        }
        if value.is_null() {
            return Ok(Self::default());
        }
        serde_yaml::from_value(value).map_err(parse)
    }

    pub fn from_ai_dir(dir: &Path) -> Result<Self, CatalogDocumentError> {
        let mut merged = Self::default();
        for file in CATALOG_FILES {
            let path = dir.join(file);
            if !path.exists() {
                continue;
            }
            let origin = path.display().to_string();
            let text =
                std::fs::read_to_string(&path).map_err(|source| CatalogDocumentError::Io {
                    origin: origin.clone(),
                    source,
                })?;
            merged.merge(Self::from_yaml_str(&origin, &text)?)?;
        }
        Ok(merged)
    }

    pub fn merge(&mut self, other: Self) -> Result<(), CatalogDocumentError> {
        merge_map(
            "credentials",
            &mut self.credentials.credentials,
            other.credentials.credentials,
        )?;
        merge_map(
            "providers",
            &mut self.providers.providers,
            other.providers.providers,
        )?;
        merge_map("models", &mut self.models.models, other.models.models)?;
        merge_map(
            "deployments",
            &mut self.deployments.deployments,
            other.deployments.deployments,
        )?;
        match (self.gateway.is_some(), other.gateway) {
            (true, Some(_)) => return Err(CatalogDocumentError::DuplicateGateway),
            (false, Some(gateway)) => self.gateway = Some(gateway),
            (_, None) => {},
        }
        Ok(())
    }

    pub fn resolve(&self, context: &CatalogContext) -> CatalogResult<ResolvedCatalog> {
        let gateway = self.gateway.clone().map(GatewayConfigSpec::resolve);
        ResolvedCatalog::build(
            CatalogInput {
                credentials: &self.credentials,
                providers: &self.providers,
                models: &self.models,
                deployments: &self.deployments,
                gateway: gateway.as_ref(),
            },
            context,
        )
    }
}

fn merge_map<K: Ord + AsRef<str>, V>(
    section: &'static str,
    into: &mut BTreeMap<K, V>,
    from: BTreeMap<K, V>,
) -> Result<(), CatalogDocumentError> {
    for (key, value) in from {
        if into.contains_key(&key) {
            return Err(CatalogDocumentError::DuplicateId {
                section,
                id: key.as_ref().to_owned(),
            });
        }
        into.insert(key, value);
    }
    Ok(())
}
