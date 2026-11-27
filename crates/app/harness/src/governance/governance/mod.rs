//! [`Governance`]: the applied instance behind the engine's Governor, and the control surface the
//! sync engine and the shell drive it with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod governor;
mod observer;
mod sessions;

pub use observer::GovernanceTelemetry;

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::extension::AppliedInstance;
use crate::projection::Applied;
use arc_swap::ArcSwapOption;
use systemprompt_traits::CancellationToken;
use indexmap::IndexMap;
use parking_lot::Mutex;
use systemprompt_device::client::CredentialSource;
use systemprompt_device::protocol::DeviceCredential;
use systemprompt_device::protocol::origin::normalize_instance_url;
use systemprompt_engine::run::extension::EnabledExtensions;
use systemprompt_identifiers::NativeSessionId;
use systemprompt_identifiers::engine::SessionId;
use tokio::sync::broadcast;
use url::Url;

use crate::governance::error::GovernanceError;
use crate::governance::hooks::HookApi;
use crate::governance::instance::Instance;
use crate::governance::telemetry::EventQueue;
use crate::governance::usage::{Counters, UsageTotals};

const BINDINGS_CAPACITY: usize = 64;

/// An instance reached through an injected [`HookApi`].
///
/// `base` is the instance's origin, which pins the gateway to `<base>/v1` and
/// decides which MCP servers may receive the session token; `extensions` is
/// the enablement the Governor serves for it.
/// [`Governance::apply`] builds one from an [`AppliedInstance`] after checking
/// that the snapshot and the client name the same instance;
/// [`Governance::install`] takes one as given.
#[derive(Clone)]
pub struct InstanceParts {
    pub applied: Applied,
    pub api: Arc<dyn HookApi>,
    pub base: Url,
    pub credentials: Arc<dyn CredentialSource>,
    pub organisation: Option<String>,
    pub extensions: Option<EnabledExtensions>,
}

impl fmt::Debug for InstanceParts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstanceParts")
            .field("instance_url", &self.applied.instance_url)
            .field("base", &self.base.as_str())
            .field("organisation", &self.organisation)
            .field("extensions", &self.extensions)
            .finish_non_exhaustive()
    }
}

/// The first governance event of a session sent with device evidence.
///
/// The sync engine posts its binding. `session_id` is the v5 UUID the instance
/// knows the session by, `native_session_id` the harness session it was derived
/// from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBinding {
    pub instance_url: String,
    pub session_id: NativeSessionId,
    pub native_session_id: SessionId,
}

pub(crate) struct Shared {
    instance: ArcSwapOption<Instance>,
    generation: AtomicU64,
    usage: Counters,
    device: Mutex<Option<DeviceCredential>>,
    verdicts: AtomicBool,
    sessions: Mutex<IndexMap<String, PathBuf>>,
    pub(crate) bindings: broadcast::Sender<SessionBinding>,
}

impl Shared {
    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    pub(crate) fn device(&self) -> Option<DeviceCredential> {
        self.device.lock().clone()
    }
}

/// The engine's Governor for the applied systemprompt instance.
///
/// Shared as an `Arc` by the engine, the sync engine and the shell. Holds the
/// instance's session-token source, hook credentials and device credential in
/// memory only.
///
/// Built inside the async runtime: its event sender is a task owned by this
/// value and a child of the given cancellation token, stopped when that token
/// fires or this value is dropped. `apply` replaces everything of the previous
/// instance (models, servers, hook client and tokens, refusals on record),
/// resets the usage totals and moves the generation on; a snapshot that names
/// another URL than its client leaves nothing applied. `clear` forgets the
/// instance. The governance extension follows the applied instance through
/// [`GovernanceApplier`](crate::governance::GovernanceApplier) and reports
/// governance telemetry through
/// [`GovernanceTelemetry`](crate::governance::GovernanceTelemetry).
pub struct Governance {
    shared: Arc<Shared>,
    events: Option<EventQueue>,
    cancel: CancellationToken,
}

impl Governance {
    pub fn new(cancel: &CancellationToken) -> Arc<Self> {
        let cancel = cancel.child_token();
        let (bindings, _) = broadcast::channel(BINDINGS_CAPACITY);
        let shared = Arc::new(Shared {
            instance: ArcSwapOption::empty(),
            generation: AtomicU64::new(0),
            usage: Counters::default(),
            device: Mutex::new(None),
            verdicts: AtomicBool::new(true),
            sessions: Mutex::new(IndexMap::new()),
            bindings,
        });
        let events = EventQueue::start(Arc::clone(&shared), cancel.clone());
        Arc::new(Self {
            shared,
            events,
            cancel,
        })
    }

    pub fn apply(&self, instance: &AppliedInstance) -> Result<(), GovernanceError> {
        let AppliedInstance {
            applied,
            client,
            credentials,
            organisation,
            extensions,
            ..
        } = instance;
        let matches =
            normalize_instance_url(&applied.instance_url).is_ok_and(|url| url == client.base());
        if !matches {
            tracing::warn!(
                "the applied snapshot and its client name different instances; nothing is applied"
            );
            self.clear();
            return Err(GovernanceError::InstanceMismatch);
        }
        let api: Arc<dyn HookApi> = Arc::new(client.clone());
        self.install(InstanceParts {
            applied: applied.clone(),
            api,
            base: client.base_url().clone(),
            credentials: Arc::clone(credentials),
            organisation: organisation.clone(),
            extensions: extensions.clone(),
        })
    }

    pub fn install(&self, parts: InstanceParts) -> Result<(), GovernanceError> {
        let epoch = self
            .shared
            .generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1);
        self.shared.usage.reset();
        match Instance::new(epoch, parts) {
            Ok(instance) => {
                tracing::info!(
                    instance = %instance.label,
                    models = instance.models.len(),
                    mcp = instance.mcp.len(),
                    governed = instance.governance_plugin().is_some(),
                    generation = epoch,
                    "instance applied to governance"
                );
                self.shared.instance.store(Some(Arc::new(instance)));
                Ok(())
            },
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "instance refused by governance; nothing is applied"
                );
                self.shared.instance.store(None);
                Err(error)
            },
        }
    }

    pub fn clear(&self) {
        let generation = self
            .shared
            .generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1);
        let previous = self.shared.instance.swap(None);
        self.shared.usage.reset();
        if previous.is_some() {
            tracing::info!(generation, "governance cleared");
        }
    }

    pub fn set_device(&self, device: Option<DeviceCredential>) {
        *self.shared.device.lock() = device;
    }

    pub fn applied(&self) -> Option<Applied> {
        self.current().map(|instance| instance.applied.clone())
    }

    pub fn usage(&self) -> UsageTotals {
        self.shared.usage.snapshot()
    }

    pub fn set_verdicts_enabled(&self, enabled: bool) {
        self.shared.verdicts.store(enabled, Ordering::SeqCst);
        tracing::info!(enabled, "tool verdicts switched");
    }

    pub fn verdicts_enabled(&self) -> bool {
        self.shared.verdicts.load(Ordering::SeqCst)
    }

    pub fn session_bindings(&self) -> broadcast::Receiver<SessionBinding> {
        self.shared.bindings.subscribe()
    }

    fn current(&self) -> Option<Arc<Instance>> {
        self.shared.instance.load_full()
    }
}

impl Drop for Governance {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(events) = &self.events {
            events.stop();
        }
    }
}

impl fmt::Debug for Governance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let instance = self.current();
        f.debug_struct("Governance")
            .field("generation", &self.shared.generation())
            .field(
                "instance",
                &instance.as_ref().map(|instance| instance.label.as_str()),
            )
            .field("verdicts", &self.verdicts_enabled())
            .finish_non_exhaustive()
    }
}
