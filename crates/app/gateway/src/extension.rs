//! The `gateway` member: the `gateway_policies` config plane, the gateway
//! slots and the one engines set of the process.
//!
//! The gateway owns no tables; the plane writes `ai_gateway_policies` through
//! the AI domain's repository. Its slots are in [`crate::contract`]; the
//! member opens each one, so every gateway slot value is built under core
//! standing and no other slot can build the same value.
//!
//! After governance it provides `Arc<GatewayEngines>` **sealed**: the
//! upstream, scanner, override and route-selector slot values, the gateway
//! guards, the kernel's sealed connector token cache and provider-key lookup,
//! and, while the profile enables the brain, the `Arc<dyn MemoryRecall>` the
//! `brain` member provides (read optionally: disabling the brain leaves the
//! engines without recall). The engines hold the provider secrets, so only
//! kernel code (the API's gateway routes) and core provide fns read them.
//! Their presence is the gateway's one owner check.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::brain::MemoryRecall;
use systemprompt_extension::prelude::*;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_security::credential::TokenCache;
use systemprompt_traits::SecretLookup;

use crate::contract::{OutboundSlot, OverrideSlot, RouteSelectorSlot, SafetyScannerSlot};
use crate::policies::GatewayPoliciesPlane;
use crate::registry::GatewayEngines;

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
        c.slot::<RouteSelectorSlot>();
        c.slot::<OverrideSlot>();
        c.slot::<SafetyScannerSlot>();
        c.slot::<OutboundSlot>();
        c.provide_conferred_after_governance(ConferralRow::GatewayEngines, provide_engines);
        Ok(())
    }
}

fn provide_engines(r: &Resolved<'_>) -> Result<Arc<GatewayEngines>, ProvideError> {
    let tokens = r.get::<Arc<TokenCache>>()?;
    let secrets = r.get::<Arc<dyn SecretLookup>>()?;
    let engines = GatewayEngines::new(tokens, r, secrets)
        .map_err(|error| ProvideError::Failed(Box::new(error)))?;
    let brain_enabled = r
        .optional::<Arc<BrainProfile>>()
        .is_some_and(|profile| profile.enabled);
    let recall = r
        .optional::<Arc<dyn MemoryRecall>>()
        .filter(|_| brain_enabled);
    Ok(Arc::new(match recall {
        Some(recall) => engines.with_memory_recall(recall),
        None => engines,
    }))
}
