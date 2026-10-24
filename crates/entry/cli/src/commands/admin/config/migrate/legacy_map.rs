//! `services/ai/legacy_map.yaml`: how the 0.72 names map onto the catalog.
//!
//! Written by `config migrate` next to the new files and read back by
//! `config migrate --data`, which moves stored personal keys and the audit's
//! `credential`/`endpoint` columns through it (`ai_catalog_legacy_map`).
//! `routes` maps every legacy route id to the id access decisions now
//! evaluate; `dropped_models` lists ids 0.72.1 could never dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::fold::Folded;

pub const LEGACY_MAP_FILE: &str = "legacy_map.yaml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyProviderTarget {
    pub provider: String,
    pub endpoint: String,
    pub credential: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyMap {
    #[serde(default)]
    pub providers: BTreeMap<String, LegacyProviderTarget>,
    #[serde(default)]
    pub routes: BTreeMap<String, String>,
    #[serde(default)]
    pub dropped_models: Vec<String>,
}

impl LegacyMap {
    #[must_use]
    pub fn new(
        folded: &Folded,
        routes: BTreeMap<String, String>,
        dropped_models: Vec<String>,
    ) -> Self {
        Self {
            providers: folded
                .entries
                .iter()
                .map(|(legacy, target)| {
                    (
                        legacy.clone(),
                        LegacyProviderTarget {
                            provider: target.endpoint.provider.as_str().to_owned(),
                            endpoint: target.endpoint.to_string(),
                            credential: target.credential.as_str().to_owned(),
                        },
                    )
                })
                .collect(),
            routes,
            dropped_models,
        }
    }
}
