//! Gateway policy routes.
//!
//! A route is policy only. Dispatch does not need one: every model resolves
//! through its own deployments (`services/ai/deployments.yaml`). A route
//! narrows or pins that chain for the requests it matches, aliases a second
//! requested id onto a catalog model, or exists to carry an audit label and an
//! access-control address. It has a declared, stable `id`; `label` is the
//! audit text and is separate from it.
//!
//! - `models`: one glob or a list (exact, prefix `foo*`, suffix `*foo`, or `*`)
//!   over catalog model ids; a route matches a model when any glob does.
//! - `when`: request predicates, including `scope` (every listed dimension must
//!   be attributed the listed value).
//! - `deployments`: an ordered subset of the matched model's own deployments
//!   (optionally weighted); empty keeps the model's chain.
//! - `context_fallbacks`: catalog models, tried in order when a request's
//!   estimated input exceeds the matched model's context window.
//! - `alias: {requested, model}`: a request for `requested` is served as
//!   `model`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod predicates;
mod selection;
mod token_estimate;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ModelId, RouteId};
use systemprompt_models::attribution::RequestAttribution;
use systemprompt_wire::canonical::CanonicalRequest;

use super::route_id::match_pattern;
use crate::services::catalog::EndpointRef;

pub use predicates::{ResponseFormatKind, RouteMatch, RouteRequirements};
pub use selection::SelectionStrategy;
pub use token_estimate::estimate_input_tokens;

/// One glob or a list of globs over catalog model ids.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(from = "OneOrMany", into = "Vec<String>")]
pub struct ModelGlobs(pub Vec<String>);

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl From<OneOrMany> for ModelGlobs {
    fn from(value: OneOrMany) -> Self {
        match value {
            OneOrMany::One(glob) => Self(vec![glob]),
            OneOrMany::Many(globs) => Self(globs),
        }
    }
}

impl From<ModelGlobs> for Vec<String> {
    fn from(value: ModelGlobs) -> Self {
        value.0
    }
}

impl ModelGlobs {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn matches(&self, model: &str) -> bool {
        self.0.iter().any(|glob| match_pattern(glob, model))
    }
}

/// One deployment a route pins, with its weight under `strategy: weighted`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum RouteDeployment {
    Endpoint(EndpointRef),
    Weighted(WeightedDeployment),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WeightedDeployment {
    pub endpoint: EndpointRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<u32>,
}

impl RouteDeployment {
    #[must_use]
    pub const fn endpoint(&self) -> &EndpointRef {
        match self {
            Self::Endpoint(endpoint) => endpoint,
            Self::Weighted(weighted) => &weighted.endpoint,
        }
    }

    #[must_use]
    pub const fn weight(&self) -> Option<u32> {
        match self {
            Self::Endpoint(_) => None,
            Self::Weighted(weighted) => weighted.weight,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteAlias {
    pub requested: ModelId,
    pub model: ModelId,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayRoute {
    pub id: RouteId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "ModelGlobs::is_empty")]
    pub models: ModelGlobs,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<RouteMatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<RouteRequirements>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deployments: Vec<RouteDeployment>,
    #[serde(default, skip_serializing_if = "SelectionStrategy::is_ordered")]
    pub strategy: SelectionStrategy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_fallbacks: Vec<ModelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<RouteAlias>,
}

impl GatewayRoute {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: RouteId::new(id),
            label: None,
            description: None,
            models: ModelGlobs::default(),
            when: None,
            requires: None,
            deployments: Vec::new(),
            strategy: SelectionStrategy::Ordered,
            context_fallbacks: Vec::new(),
            alias: None,
        }
    }

    #[must_use]
    pub fn matches_model(&self, model: &str) -> bool {
        self.models.matches(model)
    }

    #[must_use]
    pub fn aliases(&self, requested: &str) -> Option<&ModelId> {
        self.alias
            .as_ref()
            .filter(|alias| alias.requested.as_str() == requested)
            .map(|alias| &alias.model)
    }

    #[must_use]
    pub fn matches_when(
        &self,
        attribution: Option<&RequestAttribution>,
        request: Option<&CanonicalRequest>,
    ) -> bool {
        self.when
            .as_ref()
            .is_none_or(|when| when.matches(attribution, request))
    }

    #[must_use]
    pub fn audit_label(&self) -> &str {
        self.label.as_deref().unwrap_or_else(|| self.id.as_str())
    }
}
