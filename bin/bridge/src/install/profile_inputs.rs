//! The facts every host's settings are rendered from, assembled once.
//!
//! The repair path, the sync writers and the probe all render a host's
//! profile from [`ProfileGenInputs`]; [`ProfileFacts::inputs_for`] is the only
//! place those inputs are built, so what the probe expects is exactly what a
//! writer would write. It sits below the host integrations so the machine
//! policy writers here can use it too.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_models::bridge::host::HostKind;

use crate::gateway::types::BridgeProfile;
use crate::ids::{HostToken, LoopbackSecret};
use crate::install::mdm::policy::McpServerEntry;

/// Inputs a host renders its profile from.
///
/// `default_model` is the gateway's configured default only when it is one
/// of `models`. `host_token` is the [`HostToken`] derived for
/// this host from the loopback secret; no renderer ever sees the secret
/// itself, so every provisioned host presents a credential the proxy can
/// attribute to it.
#[derive(Debug, Clone)]
pub struct ProfileGenInputs {
    pub gateway_base_url: String,
    pub host_token: HostToken,
    pub models: Vec<String>,
    pub model_limits: BTreeMap<String, systemprompt_models::bridge::profile::AdvertisedLimits>,
    pub default_model: Option<String>,
    pub organization_uuid: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub mcp_servers: Option<Vec<McpServerEntry>>,
}

/// The user's per-host model filters, as the gateway stores them and serves
/// them in the signed manifest. The server is their one store.
pub type ModelProtocolOverrides = BTreeMap<HostKind, Vec<String>>;

/// Why the facts a host profile is rendered from could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ProfileFactsError {
    #[error("load config: {0}")]
    Config(#[from] crate::config::ConfigReadError),
    #[error("loopback secret: {0}")]
    LoopbackSecret(#[source] std::io::Error),
    #[error("cached bridge profile: {0}")]
    ProfileCache(#[from] crate::auth::profile_cache::ProfileCacheError),
    #[error("resolve managed MCP servers: {0}")]
    ManagedServers(#[source] std::io::Error),
    #[error("stored manifest envelope: {0}")]
    StoredManifest(#[source] std::io::Error),
    #[error("decode stored manifest: {0}")]
    ManifestDecode(#[from] crate::gateway::manifest::ManifestError),
}

/// The local facts a host profile is rendered from.
///
/// `profile` is `None` until the gateway has served one to this bridge; a
/// host then renders with no gateway models. `mcp_servers` is `None` until a
/// sync has published the registry: an empty registry then means "not
/// loaded", not "no servers", so the connector list is neither written nor
/// compared.
#[derive(Debug, Clone)]
pub struct ProfileFacts {
    pub gateway_base_url: String,
    pub secret: LoopbackSecret,
    pub profile: Option<BridgeProfile>,
    pub mcp_servers: Option<Vec<McpServerEntry>>,
    pub overrides: ModelProtocolOverrides,
}

impl ProfileFacts {
    pub fn load(
        loopback: &crate::proxy::LoopbackEndpoint,
        registry: &crate::mcp_registry::McpRegistry,
        overrides: ModelProtocolOverrides,
    ) -> Result<Self, ProfileFactsError> {
        let secret = loopback
            .secret()
            .map_err(ProfileFactsError::LoopbackSecret)?;
        let mcp_servers = if crate::mcp_registry::fragment_exists() {
            crate::install::mdm::policy::mcp_entries(loopback, registry)
                .map_err(ProfileFactsError::ManagedServers)?
        } else {
            None
        };
        Ok(Self {
            gateway_base_url: loopback.origin(),
            secret,
            profile: crate::auth::profile_cache::load()?,
            mcp_servers,
            overrides,
        })
    }

    // Why: the filters are read from the envelope the gateway signed, not
    // from a bridge-side copy; a manifest from another gateway is not this
    // user's filter set.
    pub fn persisted_overrides() -> Result<ModelProtocolOverrides, ProfileFactsError> {
        let Some(fragment) =
            crate::mcp_registry::read_envelope().map_err(ProfileFactsError::StoredManifest)?
        else {
            return Ok(ModelProtocolOverrides::new());
        };
        let cfg = crate::config::load()?;
        let gateway = crate::config::gateway_url_or_default(&cfg);
        if !crate::mcp_registry::same_origin(&fragment.gateway, &gateway) {
            return Ok(ModelProtocolOverrides::new());
        }
        Ok(crate::gateway::manifest::decode_payload(&fragment.envelope)?.host_model_protocols)
    }

    #[must_use]
    pub fn inputs_for(&self, host: HostKind) -> ProfileGenInputs {
        let clients = crate::gateway::model_view::HostClients::for_host(host, &self.overrides);
        let mut headers = BTreeMap::new();
        if !clients.families.is_empty() {
            headers.insert(
                systemprompt_identifiers::headers::INFERENCE_PROTOCOL.to_owned(),
                clients.tags().join(","),
            );
        }
        let (models, model_limits, default_model, gateway_org) = self.profile.as_ref().map_or_else(
            || (Vec::new(), BTreeMap::new(), None, None),
            |profile| {
                let models = crate::gateway::model_view::host_model_view(
                    &profile.providers,
                    &clients.families,
                )
                .compatible_models;
                (
                    models,
                    profile.model_limits.clone(),
                    profile.default_model.clone(),
                    profile.organization_uuid.clone(),
                )
            },
        );
        ProfileGenInputs {
            gateway_base_url: self.gateway_base_url.clone(),
            host_token: crate::proxy::scoped_token::host_token(&self.secret, host),
            models,
            model_limits,
            default_model,
            organization_uuid: gateway_org,
            headers,
            mcp_servers: self.mcp_servers.clone(),
        }
    }
}
