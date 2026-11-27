//! The governance extension: required whatever the instance enables. Its engine
//! half observes governed calls for telemetry, its instance half follows the
//! applied instance. The [`Governance`] both serve is the one the composition
//! root built and installed as the engine's Governor, found in the shared
//! handles; the tool verdict stays with the Governor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_engine::run::extension::GovernorObserver;
use systemprompt_identifiers::engine::extension_id;
use crate::extension::{
    AppliedInstance, ApplyError, EngineExtension, ExtensionId, ExtensionMetadata, ManifestApplier,
    SpContext, SpExtension, register_sp_extension,
};

use crate::governance::governance::{Governance, GovernanceTelemetry};

/// The governance extension, id `governance`, required.
///
/// It reads the [`Governance`] the composition root inserted into
/// `SpContext::handles` before `provide`; without one it contributes
/// nothing and says so.
#[derive(Debug, Clone, Copy, Default)]
pub struct GovernanceExtension;

impl GovernanceExtension {
    pub const ID: ExtensionId = extension_id!("governance");
}

const fn metadata() -> ExtensionMetadata {
    ExtensionMetadata::new(GovernanceExtension::ID, "Governance").mark_required()
}

register_sp_extension!(GovernanceExtension);

impl SpExtension for GovernanceExtension {
    fn metadata(&self) -> ExtensionMetadata {
        metadata()
    }

    fn engine(&self, ctx: &SpContext) -> Option<Arc<dyn EngineExtension>> {
        let governance = shared(ctx)?;
        Some(Arc::new(GovernanceEngine {
            telemetry: Arc::new(GovernanceTelemetry::new(governance)),
        }))
    }

    fn appliers(&self, ctx: &SpContext) -> Vec<Arc<dyn ManifestApplier>> {
        shared(ctx)
            .map(|governance| -> Arc<dyn ManifestApplier> {
                Arc::new(GovernanceApplier::new(governance))
            })
            .into_iter()
            .collect()
    }
}

fn shared(ctx: &SpContext) -> Option<Arc<Governance>> {
    let governance = ctx.handles.get::<Governance>();
    if governance.is_none() {
        tracing::error!(
            extension = %GovernanceExtension::ID,
            "no governance among the shared handles; the governance extension contributes nothing"
        );
    }
    governance
}

struct GovernanceEngine {
    telemetry: Arc<GovernanceTelemetry>,
}

impl EngineExtension for GovernanceEngine {
    fn metadata(&self) -> ExtensionMetadata {
        metadata()
    }

    fn observer(&self) -> Option<Arc<dyn GovernorObserver>> {
        let telemetry: Arc<dyn GovernorObserver> =
            Arc::<GovernanceTelemetry>::clone(&self.telemetry);
        Some(telemetry)
    }
}

/// Hands every applied instance to [`Governance::apply`] and every clear to
/// [`Governance::clear`].
pub struct GovernanceApplier {
    governance: Arc<Governance>,
}

impl GovernanceApplier {
    pub const fn new(governance: Arc<Governance>) -> Self {
        Self { governance }
    }
}

impl ManifestApplier for GovernanceApplier {
    fn apply(&self, instance: &AppliedInstance) -> Result<(), ApplyError> {
        self.governance.apply(instance).map_err(ApplyError::new)
    }

    fn clear(&self) {
        self.governance.clear();
    }
}

impl fmt::Debug for GovernanceApplier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceApplier")
            .field("governance", &self.governance)
            .finish()
    }
}
