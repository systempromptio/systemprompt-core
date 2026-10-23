//! An endpoint: one wire plus one URL under a provider.
//!
//! The wire, the URL, the `anthropic-beta` flags the upstream accepts, the
//! region, the price multiplier, the hosting and any resilience override are
//! facts of one endpoint, not
//! of the vendor account, so a provider serving two wires or three regions
//! declares one endpoint per wire or region under one credential. An
//! endpoint's `accepted_betas` is either its own list or `inherit:<endpoint>`,
//! the list of a sibling endpoint of the same provider, so a regional copy
//! never restates the flags by hand.
//!
//! [`EndpointRef`] is how deployments and routes name an endpoint:
//! `provider/endpoint`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{EndpointId, ProviderId};
use systemprompt_wire::anthropic::AnthropicBeta;
use systemprompt_wire::{Hosting, WireProtocol};

use crate::services::ai::ResilienceSettings;

const INHERIT_PREFIX: &str = "inherit:";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum BetaSpec {
    List(BTreeSet<AnthropicBeta>),
    Inherit(String),
}

impl BetaSpec {
    #[must_use]
    pub fn inherit(endpoint: &EndpointId) -> Self {
        Self::Inherit(format!("{INHERIT_PREFIX}{endpoint}"))
    }

    #[must_use]
    pub fn inherited_endpoint(&self) -> Option<Result<EndpointId, &str>> {
        match self {
            Self::List(_) => None,
            Self::Inherit(raw) => Some(
                raw.strip_prefix(INHERIT_PREFIX)
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .map(EndpointId::new)
                    .ok_or(raw.as_str()),
            ),
        }
    }
}

/// Where an endpoint is hosted, when the URL alone does not say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EndpointHosting {
    FirstParty,
    Vertex,
}

impl From<EndpointHosting> for Hosting {
    fn from(value: EndpointHosting) -> Self {
        match value {
            EndpointHosting::FirstParty => Self::FirstParty,
            EndpointHosting::Vertex => Self::Vertex,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EndpointConfig {
    pub wire: WireProtocol,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_betas: Option<BetaSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_multiplier: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosting: Option<EndpointHosting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resilience: Option<ResilienceSettings>,
}

impl EndpointConfig {
    #[must_use]
    pub fn new(wire: WireProtocol, url: impl Into<String>) -> Self {
        Self {
            wire,
            url: url.into(),
            accepted_betas: None,
            region: None,
            price_multiplier: None,
            hosting: None,
            resilience: None,
        }
    }

    #[must_use]
    pub fn effective_hosting(&self) -> Hosting {
        self.hosting
            .map_or_else(|| Hosting::of(&self.url), Hosting::from)
    }
}

/// A string that is not a `provider/endpoint` reference.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("endpoint reference '{raw}' is not `provider/endpoint`")]
pub struct EndpointRefError {
    pub raw: String,
}

/// `provider/endpoint`: the address of one endpoint in the catalog.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, schemars::JsonSchema)]
#[schemars(with = "String")]
pub struct EndpointRef {
    pub provider: ProviderId,
    pub endpoint: EndpointId,
}

impl EndpointRef {
    #[must_use]
    pub fn new(provider: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            provider: ProviderId::new(provider),
            endpoint: EndpointId::new(endpoint),
        }
    }

    pub fn parse(raw: &str) -> Result<Self, EndpointRefError> {
        match raw.split_once('/') {
            Some((provider, endpoint))
                if !provider.trim().is_empty()
                    && !endpoint.trim().is_empty()
                    && !endpoint.contains('/') =>
            {
                Ok(Self::new(provider.trim(), endpoint.trim()))
            },
            _ => Err(EndpointRefError {
                raw: raw.to_owned(),
            }),
        }
    }
}

impl fmt::Display for EndpointRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.provider, self.endpoint)
    }
}

impl Serialize for EndpointRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for EndpointRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}
