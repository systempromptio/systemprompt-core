//! `Extension` registration for identity issuance: the `identity` config
//! plane. This crate owns no tables; the plane writes users through the users
//! domain's repository and static OAuth clients through the OAuth domain's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::*;

use crate::identity::IdentityPlane;

#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityExtension;

impl Extension for IdentityExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "identity",
            name: "Identity",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users", "oauth"]
    }

    fn config_planes(&self) -> Vec<Arc<dyn ConfigPlane>> {
        vec![Arc::new(IdentityPlane)]
    }
}

register_extension!(IdentityExtension);
