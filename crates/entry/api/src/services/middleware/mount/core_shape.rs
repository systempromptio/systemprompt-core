//! The protection shape a core declaration row pins, and the one field a
//! declaration that names the row differs in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::FrameOptions;
use systemprompt_extension::{ProtectionLayer, RouteProtection};

use super::core_declaration::CoreDeclaration;

/// A field of the protection shape a [`CoreDeclaration`] row pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreShapeField {
    Policy,
    Context,
    RateLimit,
    BodyLimit,
    Layers,
    FrameOptions,
}

impl CoreDeclaration {
    #[must_use]
    pub const fn layers(self) -> &'static [ProtectionLayer] {
        &[]
    }

    #[must_use]
    pub const fn frame_options(self) -> Option<FrameOptions> {
        None
    }

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
        } else if protection.layers.as_slice() != self.layers() {
            Some(CoreShapeField::Layers)
        } else if protection.frame_options != self.frame_options() {
            Some(CoreShapeField::FrameOptions)
        } else {
            None
        }
    }

    pub(super) fn shape_matches(self, protection: &RouteProtection) -> bool {
        self.mismatch(protection).is_none()
    }
}
