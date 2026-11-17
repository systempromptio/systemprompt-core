//! Every admitted member's asset declarations, stamped with their owners in
//! composition order.
//!
//! The kernel builds one [`AdmittedAssets`] from the admitted composition and
//! seeds it into state, open: an asset declaration maps the caller's asset
//! paths to source and destination files and carries no secret, so a
//! member's command (`cloud deploy`'s artifact check) evaluates it against
//! the project being deployed without naming the admitted composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;

use crate::admitted::BuiltState;
use crate::asset::{AssetDefinition, AssetPaths};
use crate::contribute::AssetFn;

/// The owner-stamped asset declarations of an admitted composition.
#[derive(Clone, Default)]
pub struct AdmittedAssets {
    assets: Vec<(ExtensionId, AssetFn)>,
}

impl fmt::Debug for AdmittedAssets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdmittedAssets")
            .field(
                "owners",
                &self
                    .assets
                    .iter()
                    .map(|(owner, _)| owner)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl AdmittedAssets {
    #[must_use]
    pub fn of(built: &BuiltState) -> Self {
        let assets = built
            .members()
            .flat_map(|member| {
                member
                    .sink
                    .assets()
                    .iter()
                    .map(|assets| (member.owner.clone(), Arc::clone(assets)))
            })
            .collect();
        Self { assets }
    }

    #[must_use]
    pub fn required(&self, paths: &dyn AssetPaths) -> Vec<(ExtensionId, AssetDefinition)> {
        self.assets
            .iter()
            .flat_map(|(owner, assets)| {
                assets(paths)
                    .into_iter()
                    .map(move |asset| (owner.clone(), asset))
            })
            .collect()
    }
}
