//! The protection shape a core declaration row pins, and the one field a
//! declaration from that row's owner differs in. An owner may list several
//! rows at one base path and group (one per shape); a declaration matching
//! any of them is admitted, and one matching none is reported against the
//! first.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::FrameOptions;
use systemprompt_extension::{ExtensionRouter, ProtectionLayer, RouteGroup, RouteProtection};

use super::core_declaration::{CoreDeclaration, CoreDeclarer};

/// A field of the protection shape a [`CoreDeclaration`] row pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreShapeField {
    BasePath,
    Group,
    Policy,
    Context,
    RateLimit,
    BodyLimit,
    Layers,
    FrameOptions,
}

impl CoreDeclaration {
    pub const PINNED_LAYERS: &'static [ProtectionLayer] = &[];

    pub const PINNED_FRAME_OPTIONS: Option<FrameOptions> = None;

    #[must_use]
    pub fn mismatch(self, protection: &RouteProtection) -> Option<CoreShapeField> {
        let policy = self.policy();
        if protection.policy.allowed() != policy.allowed()
            || protection.policy.handler_verifier() != policy.handler_verifier()
        {
            Some(CoreShapeField::Policy)
        } else if protection.context != self.context() {
            Some(CoreShapeField::Context)
        } else if protection.rate_limit != Some(self.rate_limit()) {
            Some(CoreShapeField::RateLimit)
        } else if protection.body_limit != self.body_limit() {
            Some(CoreShapeField::BodyLimit)
        } else if protection.layers.as_slice() != Self::PINNED_LAYERS {
            Some(CoreShapeField::Layers)
        } else if protection.frame_options != Self::PINNED_FRAME_OPTIONS {
            Some(CoreShapeField::FrameOptions)
        } else {
            None
        }
    }

    pub(super) fn owner_drift(
        owner: CoreDeclarer,
        declared: &ExtensionRouter,
    ) -> Option<CoreShapeField> {
        let rows = || {
            Self::ALL.iter().filter(move |entry| {
                entry.declarer() == owner && entry.group() != RouteGroup::Gateway
            })
        };
        rows().next()?;
        let named =
            || rows().filter(|entry| entry.names(owner, declared.base_path, declared.group));
        if named().any(|row| row.shape_matches(&declared.protection)) {
            return None;
        }
        match named().next() {
            Some(row) => row.mismatch(&declared.protection),
            None if rows().any(|entry| entry.base_path() == declared.base_path) => {
                Some(CoreShapeField::Group)
            },
            None => Some(CoreShapeField::BasePath),
        }
    }

    pub(super) fn shape_matches(self, protection: &RouteProtection) -> bool {
        self.mismatch(protection).is_none()
    }
}
