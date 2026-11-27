//! Where accepted content goes: the applier and the extensions' manifest
//! appliers.
//!
//! The applier runs one blocking job at a time on a blocking thread; the
//! manifest appliers take the applied instance (governance among them, serving
//! it to the engine). Device evidence, usage totals and session bindings stay
//! governance's own.
//!
//! A job holds the applier's gate until its blocking thread returns, so a clear
//! queued after an apply whose caller went away still runs after it, never
//! beside it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::sync::Arc;

use crate::extension::{AppliedInstance, EnabledExtensions, ManifestAppliers};
use crate::governance::{Governance, SessionBinding, UsageTotals};
use crate::projection::{Applied, Applier, ApplyReport, Projection};
use systemprompt_device::accounts::Accounts;
use systemprompt_device::protocol::DeviceCredential;
use systemprompt_device::protocol::ids::AccountId;
use systemprompt_models::feedback::receipts::{ConsumerInstallationPlan, RuntimeFileReadback};
use tokio::sync::{Mutex, broadcast};

pub use crate::sync::error::TargetError;

/// Where accepted content goes: the applier, the extensions' manifest appliers
/// and governance; implemented by the live target the engine builds from an
/// `Applier`, the `ManifestAppliers` and `Governance`.
///
/// `govern` hands the applied instance, with the extensions its manifest
/// enabled for this host, to every admitted extension; `clear` clears them.
pub trait Target: Clone + Send + Sync + 'static {
    fn apply(
        &self,
        projection: Projection,
    ) -> impl Future<Output = Result<ApplyReport, TargetError>> + Send;

    fn clear(&self) -> impl Future<Output = ()> + Send;

    fn govern(
        &self,
        account: &AccountId,
        applied: Applied,
        extensions: Option<EnabledExtensions>,
    ) -> Result<(), TargetError>;

    fn materialize(
        &self,
        skill_dir: String,
        plan: ConsumerInstallationPlan,
    ) -> impl Future<Output = Result<Vec<RuntimeFileReadback>, TargetError>> + Send;

    fn inspect(
        &self,
        skill_dir: String,
        plan: ConsumerInstallationPlan,
    ) -> impl Future<Output = bool> + Send;

    fn set_device(&self, device: Option<DeviceCredential>);

    fn usage(&self) -> UsageTotals;

    fn session_bindings(&self) -> broadcast::Receiver<SessionBinding>;
}

#[derive(Debug, Clone)]
pub(crate) struct Gated {
    applier: Applier,
    gate: Arc<Mutex<()>>,
}

impl Gated {
    pub(crate) fn new(applier: Applier) -> Self {
        Self {
            applier,
            gate: Arc::new(Mutex::new(())),
        }
    }

    pub(crate) async fn run<T, F>(&self, job: F) -> Result<T, TargetError>
    where
        F: FnOnce(&Applier) -> T + Send + 'static,
        T: Send + 'static,
    {
        let held = Arc::clone(&self.gate).lock_owned().await;
        let applier = self.applier.clone();
        tokio::task::spawn_blocking(move || {
            let _held = held;
            job(&applier)
        })
        .await
        .map_err(TargetError::Blocking)
    }
}

#[derive(Clone)]
pub(crate) struct LiveTarget {
    applier: Gated,
    appliers: Arc<ManifestAppliers>,
    governance: Arc<Governance>,
    accounts: Accounts,
}

impl LiveTarget {
    pub(crate) fn new(
        applier: Applier,
        appliers: ManifestAppliers,
        governance: Arc<Governance>,
        accounts: Accounts,
    ) -> Self {
        Self {
            applier: Gated::new(applier),
            appliers: Arc::new(appliers),
            governance,
            accounts,
        }
    }
}

impl std::fmt::Debug for LiveTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveTarget")
            .field("applier", &self.applier)
            .field("appliers", &self.appliers)
            .finish_non_exhaustive()
    }
}

impl Target for LiveTarget {
    async fn apply(&self, projection: Projection) -> Result<ApplyReport, TargetError> {
        self.applier
            .run(move |applier| applier.apply(&projection))
            .await?
            .map_err(TargetError::Projection)
    }

    async fn clear(&self) {
        self.appliers.clear();
        self.governance.set_device(None);
        match self.applier.run(Applier::clear).await {
            Ok(Ok(())) => {},
            Ok(Err(error)) => tracing::warn!(error = %error, "clearing the managed content failed"),
            Err(error) => {
                tracing::warn!(error = %error, "clearing the managed content did not finish");
            },
        }
    }

    fn govern(
        &self,
        account: &AccountId,
        applied: Applied,
        extensions: Option<EnabledExtensions>,
    ) -> Result<(), TargetError> {
        let client = self
            .accounts
            .client(account)
            .map_err(TargetError::Accounts)?;
        let organisation = self.accounts.get(account).and_then(|view| view.org);
        let instance = AppliedInstance {
            account: account.clone(),
            applied,
            client,
            credentials: self.accounts.credentials(account),
            organisation,
            extensions,
        };
        self.appliers
            .apply(&instance)
            .map_err(TargetError::Extension)
    }

    async fn materialize(
        &self,
        skill_dir: String,
        plan: ConsumerInstallationPlan,
    ) -> Result<Vec<RuntimeFileReadback>, TargetError> {
        self.applier
            .run(move |applier| applier.materialize(&skill_dir, &plan))
            .await?
            .map_err(TargetError::Materialize)
    }

    async fn inspect(&self, skill_dir: String, plan: ConsumerInstallationPlan) -> bool {
        match self
            .applier
            .run(move |applier| applier.inspect(&skill_dir, &plan))
            .await
        {
            Ok(intact) => intact,
            Err(error) => {
                tracing::warn!(error = %error, "inspecting runtime files did not finish");
                false
            },
        }
    }

    fn set_device(&self, device: Option<DeviceCredential>) {
        self.governance.set_device(device);
    }

    fn usage(&self) -> UsageTotals {
        self.governance.usage()
    }

    fn session_bindings(&self) -> broadcast::Receiver<SessionBinding> {
        self.governance.session_bindings()
    }
}
