//! One applied instance: its projection snapshot, the model and server lists
//! derived from it, the extensions it enabled, the client and token source
//! that reach it, and its in-memory hook credentials. Replacing the instance
//! drops all of it at once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use harness_engine::extension::EnabledExtensions;
use harness_engine::{McpSpec, Purpose};
use harness_ids::SessionId;
use harness_llm::ModelSpec;
use parking_lot::Mutex;
use sp_client::{CredentialSource, HookClient, HookGrant};
use sp_projection::Applied;
use sp_protocol::HookToken;
use sp_protocol::ids::PluginId;
use sp_protocol::origin::is_transport_secure;
use url::Url;

use crate::error::GovernanceError;
use crate::governance::InstanceParts;
use crate::hooks::HookApi;
use crate::{mcp, models};

pub(crate) const JWT_MIN_VALIDITY: Duration = Duration::from_secs(60);

const REMEMBERED: usize = 4_096;

pub(crate) type RefusalKey = (String, Purpose);

pub(crate) struct Instance {
    pub(crate) epoch: u64,
    pub(crate) applied: Applied,
    pub(crate) api: Arc<dyn HookApi>,
    pub(crate) credentials: Arc<dyn CredentialSource>,
    pub(crate) organisation: Option<String>,
    pub(crate) extensions: Option<EnabledExtensions>,
    pub(crate) label: String,
    pub(crate) base_url: Url,
    pub(crate) protocols: Option<String>,
    pub(crate) models: Vec<ModelSpec>,
    pub(crate) mcp: Vec<McpSpec>,
    pub(crate) mint_gate: tokio::sync::Mutex<()>,
    hooks: Mutex<HookState>,
    refused: Mutex<HashSet<RefusalKey>>,
    stale_jwt: AtomicBool,
    bound: Mutex<HashSet<SessionId>>,
}

#[derive(Default)]
struct HookState {
    client: Option<HookClient>,
    grants: HashMap<String, HookGrant>,
}

impl Instance {
    pub(crate) fn new(epoch: u64, parts: InstanceParts) -> Result<Self, GovernanceError> {
        let InstanceParts {
            applied,
            api,
            base,
            credentials,
            organisation,
            extensions,
        } = parts;
        if !is_transport_secure(&base) {
            return Err(GovernanceError::InsecureInstance);
        }
        let base_url = Url::parse(&format!("{}/v1", base.as_str().trim_end_matches('/')))
            .map_err(|_unparsable| GovernanceError::InstanceMismatch)?;
        if let Some(declared) = applied.gateway.base_url.as_deref()
            && declared.trim_end_matches('/') != base_url.as_str().trim_end_matches('/')
        {
            tracing::warn!(
                declared,
                pinned = %base_url,
                "the projected gateway URL differs from the instance; calls stay on the instance"
            );
        }
        let label = base.host_str().unwrap_or(base.as_str()).to_owned();
        let protocols =
            (!applied.gateway.protocols.is_empty()).then(|| applied.gateway.protocols.join(","));
        let models = applied.gateway.models.iter().map(models::spec).collect();
        let mcp = mcp::specs(&applied.mcp, &base, &credentials);
        let organisation = organisation
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty());
        Ok(Self {
            epoch,
            applied,
            api,
            credentials,
            organisation,
            extensions,
            label,
            base_url,
            protocols,
            models,
            mcp,
            mint_gate: tokio::sync::Mutex::new(()),
            hooks: Mutex::new(HookState::default()),
            refused: Mutex::new(HashSet::new()),
            stale_jwt: AtomicBool::new(false),
            bound: Mutex::new(HashSet::new()),
        })
    }

    pub(crate) fn offers(&self, model: &str) -> bool {
        self.models.iter().any(|spec| spec.id == model)
    }

    pub(crate) const fn governance_plugin(&self) -> Option<&PluginId> {
        self.applied.governance_plugin.as_ref()
    }

    pub(crate) fn cached_token(&self, plugin: &PluginId, now: DateTime<Utc>) -> Option<HookToken> {
        let hooks = self.hooks.lock();
        hooks
            .grants
            .get(plugin.as_str())
            .filter(|grant| !grant.needs_refresh(now))
            .map(|grant| grant.token.clone())
    }

    pub(crate) fn cached_client(&self) -> Option<HookClient> {
        self.hooks.lock().client.clone()
    }

    pub(crate) fn keep_client(&self, client: HookClient) {
        self.hooks.lock().client = Some(client);
    }

    pub(crate) fn keep_grant(&self, plugin: &PluginId, grant: HookGrant) {
        self.hooks
            .lock()
            .grants
            .insert(plugin.as_str().to_owned(), grant);
    }

    // Why: the engine counts a step's sends from 1, so its first send is always a
    // first refusal; a later send is one only when no refusal of the same step
    // is on record.
    pub(crate) fn first_refusal(&self, key: RefusalKey, attempt: u32) -> bool {
        let unrecorded = {
            let mut refused = self.refused.lock();
            if refused.len() >= REMEMBERED {
                refused.clear();
            }
            refused.insert(key)
        };
        unrecorded || attempt <= 1
    }

    pub(crate) fn forget_refusal(&self, key: &RefusalKey) {
        self.refused.lock().remove(key);
    }

    pub(crate) fn mark_stale(&self) {
        self.stale_jwt.store(true, Ordering::SeqCst);
    }

    pub(crate) fn take_stale(&self) -> bool {
        self.stale_jwt.swap(false, Ordering::SeqCst)
    }

    pub(crate) fn first_binding(&self, native_session: &SessionId) -> bool {
        let mut bound = self.bound.lock();
        if bound.len() >= REMEMBERED {
            bound.clear();
        }
        bound.insert(native_session.clone())
    }
}

impl fmt::Debug for Instance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Instance")
            .field("epoch", &self.epoch)
            .field("label", &self.label)
            .field("base_url", &self.base_url.as_str())
            .field("models", &self.models.len())
            .field("mcp", &self.mcp.len())
            .field("governance_plugin", &self.applied.governance_plugin)
            .field("extensions", &self.extensions)
            .finish_non_exhaustive()
    }
}
