//! Lifecycle wrapper for the gateway section of a profile.
//!
//! YAML deserialization always produces [`GatewayState::Spec`]; the
//! profile loader projects it to [`GatewayState::Resolved`]. Runtime read
//! paths must observe [`GatewayState::Resolved`] — they consult
//! [`Self::resolved`] which logs and returns `None` if the loader has not run.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use systemprompt_identifiers::RouteId;

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

    /// Every route id the gateway can dispatch to: the content-addressed ids of
    /// the explicit routes, plus the synthetic catch-all route to
    /// `default_provider` when one is set. Works in either lifecycle state, so
    /// a caller can materialise the authz entity catalog straight from the
    /// typed profile without having run the loader.
    ///
    /// This is the single source of truth the authz entity catalog is
    /// materialised from (the bootstrap job and `admin config reconcile` both
    /// call it), and it MUST mirror
    /// [`super::config::GatewayConfig::resolve_route`]'s reachable set —
    /// otherwise a model the resolver can serve via the synthetic
    /// default route would be denied as an unknown entity.
    #[must_use]
    pub fn resolved_route_ids(&self) -> Vec<RouteId> {
        let mut spec = self.clone().into_spec();
        let mut ids: Vec<RouteId> = spec
            .routes
            .iter_mut()
            .map(|route| {
                route.ensure_id();
                route.id.clone()
            })
            .collect();
        if let Some(default_id) = spec.default_route_id() {
            if !ids.contains(&default_id) {
                ids.push(default_id);
            }
        }
        ids
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
    fn schema_name() -> std::borrow::Cow<'static, str> {
        GatewayConfigSpec::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        GatewayConfigSpec::json_schema(generator)
    }
}
