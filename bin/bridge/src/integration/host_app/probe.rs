//! Context supplied to host configuration probes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::{HostApp, HostAppError, ProfileFacts, ProfileFactsError};
use crate::drift::DesiredProfile;

/// What a host probe needs to judge a profile current: the facts every
/// host's settings are rendered from, and the proxy port for display.
///
/// `facts` is an `Err` when they could not be read; every probe then reports
/// its profile unverifiable rather than current. Tests build the value
/// directly with whatever facts they need.
#[derive(Debug, Clone)]
pub struct ProbeEnv {
    pub proxy_port: u16,
    pub start_menu: Arc<crate::probe_cache::StartMenuCache>,
    pub facts: Result<ProfileFacts, Arc<ProfileFactsError>>,
    pub policy_writer_ready: bool,
}

/// Why a host's expected settings could not be built for comparison.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DesiredError {
    #[error(transparent)]
    Facts(Arc<ProfileFactsError>),
    #[error(transparent)]
    Render(Arc<HostAppError>),
}

impl ProbeEnv {
    #[must_use]
    pub fn for_bridge(bridge: &crate::context::BridgeContext) -> Self {
        let loopback = bridge.proxy.loopback();
        let facts = ProfileFacts::persisted_overrides()
            .and_then(|overrides| ProfileFacts::load(loopback, &bridge.mcp_registry(), overrides))
            .map_err(|error| {
                tracing::warn!(error = %error, "host profile facts are unreadable; host probes report their settings as unverifiable");
                Arc::new(error)
            });
        Self {
            proxy_port: loopback.port(),
            start_menu: Arc::clone(&bridge.start_menu),
            facts,
            policy_writer_ready: false,
        }
        .with_policy_writer()
    }

    // Why: whether a rewrite of the machine policy will raise the
    // administrator prompt depends on the writer being registered and
    // usable; the probe records it so the verb the GUI offers is right.
    #[cfg(target_os = "windows")]
    #[must_use]
    pub fn with_policy_writer(mut self) -> Self {
        self.policy_writer_ready = matches!(
            crate::install::policy_writer::status(),
            crate::install::policy_writer::WriterStatus::Ready
        );
        self
    }

    #[cfg(not(target_os = "windows"))]
    #[must_use]
    pub const fn with_policy_writer(self) -> Self {
        self
    }

    pub fn desired_for(&self, host: &dyn HostApp) -> Result<DesiredProfile, DesiredError> {
        let facts = self
            .facts
            .as_ref()
            .map_err(|e| DesiredError::Facts(Arc::clone(e)))?;
        host.desired(&facts.inputs_for(host.id()))
            .map_err(|e| DesiredError::Render(Arc::new(e)))
    }
}
