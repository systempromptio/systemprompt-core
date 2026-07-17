//! Lifecycle wrapper for the gateway section of a profile.
//!
//! YAML deserialization always produces [`GatewayState::Spec`]; the
//! profile loader projects it to [`GatewayState::Resolved`]. Runtime read
//! paths must observe [`GatewayState::Resolved`] — they consult
//! [`Self::resolved`] which logs and returns `None` if the loader has not run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use systemprompt_identifiers::RouteId;

use super::super::providers::ProviderRegistry;
use super::config::{GatewayConfig, GatewayConfigSpec};

#[derive(Debug, Clone)]
pub enum GatewayState {
    Spec(GatewayConfigSpec),
    Resolved(GatewayConfig),
}

impl GatewayState {
    /// The resolved runtime config, or `None` if the loader did not run.
    /// A `None` here is a bootstrap-ordering bug; the log line is the
    /// signal — production read paths fall through to the same "gateway
    /// absent" path they already handle.
    #[must_use]
    pub fn resolved(&self) -> Option<&GatewayConfig> {
        match self {
            Self::Resolved(c) => Some(c),
            Self::Spec(_) => {
                tracing::error!(
                    "gateway state is still Spec at runtime read; GatewayConfigSpec::resolve was \
                     never called — treating gateway as absent"
                );
                None
            },
        }
    }

    #[must_use]
    pub const fn as_spec_mut(&mut self) -> Option<&mut GatewayConfigSpec> {
        match self {
            Self::Spec(s) => Some(s),
            Self::Resolved(_) => None,
        }
    }

    pub fn into_spec(self) -> GatewayConfigSpec {
        match self {
            Self::Spec(s) => s,
            Self::Resolved(c) => c.to_spec(),
        }
    }

    #[must_use]
    pub fn dispatchable_route_ids(&self, registry: &ProviderRegistry) -> Vec<RouteId> {
        let config = match self {
            Self::Resolved(c) => Cow::Borrowed(c),
            Self::Spec(s) => Cow::Owned(s.clone().resolve()),
        };
        config.dispatchable_route_ids(registry)
    }
}

impl<'de> Deserialize<'de> for GatewayState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        GatewayConfigSpec::deserialize(deserializer).map(Self::Spec)
    }
}

impl Serialize for GatewayState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Spec(s) => s.serialize(serializer),
            Self::Resolved(c) => c.to_spec().serialize(serializer),
        }
    }
}

impl schemars::JsonSchema for GatewayState {
    fn schema_name() -> Cow<'static, str> {
        GatewayConfigSpec::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        GatewayConfigSpec::json_schema(generator)
    }
}
