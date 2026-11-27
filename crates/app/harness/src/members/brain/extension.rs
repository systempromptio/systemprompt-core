//! [`BrainExtension`]: the brain's instance-side half, named in the
//! registration list of [`crate::extension`].
//!
//! `provide` publishes the [`BrainService`] in the shared handles, where the
//! shell half finds it; the engine half, the applier and the probe are built
//! over that one service. The applier attaches the applied instance's client
//! while the instance admits the extension and detaches it otherwise, so a
//! brain the instance did not enable for this host is never reached.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::extension::{
    AppliedInstance, ApplyError, BackgroundService, EngineExtension, ExtensionId,
    ExtensionMetadata, ManifestApplier, SpContext, SpExtension, extension_id,
};

use crate::members::brain::client::BrainClient;
use crate::members::brain::engine::BrainEngine;
use crate::members::brain::service::{BrainProbe, BrainService};

pub const BRAIN_EXTENSION: ExtensionId = extension_id!("brain");

const NAME: &str = "Brain";

pub(crate) const fn metadata() -> ExtensionMetadata {
    ExtensionMetadata::new(BRAIN_EXTENSION, NAME)
}

/// The brain extension: team memory over the instance's brain API.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrainExtension;


impl SpExtension for BrainExtension {
    fn metadata(&self) -> ExtensionMetadata {
        metadata()
    }

    fn provide(&self, ctx: &SpContext) {
        ctx.handles
            .insert(Arc::new(BrainService::new(ctx.http.clone())));
    }

    fn engine(&self, ctx: &SpContext) -> Option<Arc<dyn EngineExtension>> {
        let service = ctx.handles.get::<BrainService>()?;
        let settings = ctx.settings_for(&BRAIN_EXTENSION);
        Some(Arc::new(BrainEngine::new(
            BrainService::clone(&service),
            settings,
        )))
    }

    fn appliers(&self, ctx: &SpContext) -> Vec<Arc<dyn ManifestApplier>> {
        ctx.handles
            .get::<BrainService>()
            .map(|service| {
                let applier: Arc<dyn ManifestApplier> = Arc::new(BrainApplier {
                    service: BrainService::clone(&service),
                });
                applier
            })
            .into_iter()
            .collect()
    }

    fn services(&self, ctx: &SpContext) -> Vec<Arc<dyn BackgroundService>> {
        ctx.handles
            .get::<BrainService>()
            .map(|service| {
                let probe: Arc<dyn BackgroundService> =
                    Arc::new(BrainProbe::new(BrainService::clone(&service)));
                probe
            })
            .into_iter()
            .collect()
    }
}

struct BrainApplier {
    service: BrainService,
}

impl ManifestApplier for BrainApplier {
    fn apply(&self, instance: &AppliedInstance) -> Result<(), ApplyError> {
        let client = BrainClient::new(
            &instance.client,
            self.service.http().clone(),
            Arc::clone(&instance.credentials),
        );
        self.service.attach(client);
        Ok(())
    }

    fn clear(&self) {
        self.service.detach();
    }
}
