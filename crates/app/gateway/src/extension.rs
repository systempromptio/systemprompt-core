//! The `gateway` member: the `gateway_policies` config plane. The gateway owns
//! no tables; the plane writes `ai_gateway_policies` through the AI domain's
//! repository. Its slots are in [`crate::contract`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::*;

use crate::policies::GatewayPoliciesPlane;

#[derive(Debug, Clone, Copy, Default)]
pub struct GatewayExtension;

impl Extension for GatewayExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "gateway",
            name: "Gateway",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.config_plane(Arc::new(GatewayPoliciesPlane));
        Ok(())
    }
}
