//! The trait an instance-side extension implements.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use harness_engine::extension::{EngineExtension, ExtensionMetadata};

use crate::applier::ManifestApplier;
use crate::context::SpContext;
use crate::service::BackgroundService;

/// A feature on the instance side of the app.
///
/// `Default`-constructed by
/// [`register_sp_extension!`](crate::register_sp_extension) and held as `dyn
/// SpExtension`. Only `metadata` is required. The composition root first calls
/// `provide` on every resolved extension, each after the extensions it depends
/// on (insert the handles the extension shares with its shell half and with
/// others into `ctx.handles`), then `engine`, `appliers` and `services` in
/// resolved order. The engine half's metadata carries the same id as this
/// one.
pub trait SpExtension: Send + Sync + 'static {
    fn metadata(&self) -> ExtensionMetadata;

    fn provide(&self, _ctx: &SpContext) {}

    fn engine(&self, _ctx: &SpContext) -> Option<Arc<dyn EngineExtension>> {
        None
    }

    fn appliers(&self, _ctx: &SpContext) -> Vec<Arc<dyn ManifestApplier>> {
        Vec::new()
    }

    fn services(&self, _ctx: &SpContext) -> Vec<Arc<dyn BackgroundService>> {
        Vec::new()
    }
}
